#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Id {
    Number(u64),
    String(String),
    Null,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub jsonrpc: String,
    pub id: Id,
    pub method: String,
    pub params_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub jsonrpc: String,
    pub id: Id,
    pub result_json: Option<String>,
    pub error: Option<Error>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub code: i64,
    pub message: String,
}

impl Request {
    pub fn new(id: Id, method: impl Into<String>, params_json: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.into(),
            params_json: params_json.into(),
        }
    }
}

impl Response {
    pub fn success(id: Id, result_json: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result_json: Some(result_json.into()),
            error: None,
        }
    }

    pub fn error(id: Id, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result_json: None,
            error: Some(Error {
                code,
                message: message.into(),
            }),
        }
    }
}
