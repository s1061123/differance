// skel.rs: which contains CmdArgs/NetConf structure for CNI
// 2022, Tomofumi Hayashi
use crate::types::types_020::CNI020Result;
use crate::types::types_040::CNI040Result;
use crate::types::types_100::CNI100Result;
use crate::types::types_common::DNS;
use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::{env, io};
use thiserror::Error;

pub struct CmdArgs {
    pub container_id: String,
    pub netns: String,
    pub ifname: String,
    pub args: HashMap<String, String>,
    pub path: String,
    pub stdin_data: String,
}

#[derive(Deserialize, Debug)]
pub struct NetConf {
    #[allow(unused)]
    #[serde(rename = "cniVersion", default)]
    pub cni_version: String,
    #[allow(unused)]
    #[serde(rename = "name", default)]
    pub name: String,
    #[allow(unused)]
    #[serde(rename = "type", default)]
    pub r#type: String,
    #[allow(unused)]
    #[serde(rename = "plugins", default, skip_serializing_if = "Vec::is_empty")]
    pub plugins: Box<Vec<NetConf>>,
    #[allow(unused)]
    #[serde(rename = "capabilities", default)]
    pub capabilities: HashMap<String, bool>,
    #[allow(unused)]
    #[serde(rename = "dns", default)]
    pub dns: DNS,
    #[allow(unused)]
    #[serde(rename = "ipam", default)]
    pub ipam: serde_json::Value,
    #[allow(unused)]
    #[serde(rename = "prevResult", default)]
    pub prev_result: serde_json::Value,
}

impl NetConf {
    //strategy in golang CNI
    //- check cniVersion and put into Result (if there is no cniVersion in result)
    //- regenerate bytes
    //- parse it again based on above cniVersion
    pub fn get_current_result(&self) -> Result<CNI100Result> {
        let cni_version = match self.prev_result["cniVersion"] {
            Value::Null => self.cni_version.clone(),
            _ => self.prev_result["cniVersion"].to_string().replace('\"', ""),
        };
        let mut result = self.prev_result.clone();
        result["cniVersion"] = serde_json::Value::String(cni_version.clone());
        let result_str_buf = serde_json::to_string(&result)?; // need to handle error
        match cni_version.as_str() {
            "0.1.0" | "0.2.0" => Ok(
                serde_json::from_str::<CNI020Result>(result_str_buf.as_str())?.convert_to_latest(),
            ),
            "0.3.0" | "0.3.1" | "0.4.0" => Ok(serde_json::from_str::<CNI040Result>(
                result_str_buf.as_str(),
            )?
            .convert_to_latest()),
            "1.0.0" => Ok(serde_json::from_str::<CNI100Result>(
                result_str_buf.as_str(),
            )?),
            _ => Err(anyhow!("failed")),
        }
    }

    pub fn get_result_output(&self, result: &CNI100Result) -> Result<String, ResultError> {
        let cni_version = self.cni_version.to_string().replace('\"', "");

        match cni_version.as_str() {
            "0.1.0" | "0.2.0" => Ok(serde_json::to_string(&CNI020Result::convert_from_latest(
                result,
                cni_version.as_str(),
            ))?),
            "0.3.0" | "0.3.1" | "0.4.0" => Ok(serde_json::to_string(
                &CNI040Result::convert_from_latest(result, cni_version.as_str()),
            )?),
            "1.0.0" => Ok(serde_json::to_string(&result)?),
            err => Err(ResultError::CNIVersionError(anyhow!("failed: {}", err))),
        }
    }
}

#[derive(Debug, Error)]
pub enum CmdArgsError<'a> {
    #[error("failed to read stdin: {0}")]
    FailedReadStdIn(io::Error),
    #[error("'{0}' is required but cannot find: {1}")]
    MissingArgs(&'a str, env::VarError),
}

impl From<io::Error> for CmdArgsError<'static> {
    fn from(err: io::Error) -> CmdArgsError<'static> {
        CmdArgsError::FailedReadStdIn(err)
    }
}

pub fn get_cmdargs_env<'a>(
    command: &str,
    arg_name: &'a str,
    required_command: (bool, bool, bool),
) -> Result<String, CmdArgsError<'a>> {
    let (add, check, del) = required_command;
    Ok(match env::var(arg_name) {
        Ok(v) => v,
        Err(err) => {
            if command == "ADD" && add {
                return Err(CmdArgsError::MissingArgs(arg_name, err));
            }
            if command == "CHECK" && check {
                return Err(CmdArgsError::MissingArgs(arg_name, err));
            }
            if command == "DEL" && del {
                return Err(CmdArgsError::MissingArgs(arg_name, err));
            }
            "".to_string()
        }
    })
}

//K=V;K2=V2;
pub fn get_args(args: &str) -> HashMap<String, String> {
    let mut args_map = HashMap::new();

    args.split(';').for_each(|r| {
        let v: Vec<&str> = r.split('=').collect();
        match v.len() {
            1 => args_map.insert(v[0].clone().to_string(), "".to_string()),
            2 => args_map.insert(v[0].clone().to_string(), v[1].clone().to_string()),
            _ => None,
        };
    });
    args_map
}

#[test]
fn test_get_args() {
    let args_map = get_args("K=V;K2=V2");

    assert_eq!(args_map[&"K".to_string()], "V");
    assert_eq!(args_map[&"K2".to_string()], "V2");
}

#[test]
fn test_get_args2() {
    let args_map = get_args("K=V;K2=V2;");

    assert_eq!(args_map.get(&"K".to_string()).unwrap(), "V");
    assert_eq!(args_map[&"K".to_string()], "V");
    assert_eq!(args_map[&"K2".to_string()], "V2");
}

pub fn get_cmdargs() -> Result<(String, CmdArgs), CmdArgsError<'static>> {
    let mut stdin = String::new();
    io::BufReader::new(io::stdin()).read_to_string(&mut stdin)?;

    let command = match env::var("CNI_COMMAND") {
        Ok(v) => v,
        Err(err) => return Err(CmdArgsError::MissingArgs("CNI_COMMAND", err)),
    };

    let args = CmdArgs {
        container_id: get_cmdargs_env(command.as_str(), "CNI_CONTAINERID", (true, true, true))?,
        netns: get_cmdargs_env(command.as_str(), "CNI_NETNS", (true, true, false))?,
        ifname: get_cmdargs_env(command.as_str(), "CNI_IFNAME", (true, true, true))?,
        args: get_args(&get_cmdargs_env(command.as_str(), "CNI_ARGS", (false, false, false))?),
        path: get_cmdargs_env(command.as_str(), "CNI_PATH", (true, true, true))?,
        stdin_data: stdin,
    };
    Ok((command, args))
}

pub fn get_netconf(stdin_str: &str) -> Result<NetConf, serde_json::Error> {
    serde_json::from_str(stdin_str)
}

#[derive(Debug, Error)]
pub enum ResultError {
    #[error("failed to encode json: {0}")]
    JsonEncodeError(serde_json::Error),
    #[error("failed to get cniVersion: {0}")]
    CNIVersionError(anyhow::Error),
}

impl From<serde_json::Error> for ResultError {
    fn from(err: serde_json::Error) -> ResultError {
        ResultError::JsonEncodeError(err)
    }
}

impl From<anyhow::Error> for ResultError {
    fn from(err: anyhow::Error) -> ResultError {
        ResultError::CNIVersionError(err)
    }
}
