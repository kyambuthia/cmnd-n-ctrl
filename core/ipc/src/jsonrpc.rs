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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_and_response_constructors() {
        let req = Request::new(Id::Number(7), "tools.list", "{}");
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "tools.list");

        let ok = Response::success(Id::Number(7), "[]");
        assert!(ok.error.is_none());
        assert_eq!(ok.result_json.as_deref(), Some("[]"));

        let err = Response::error(Id::Number(7), -32601, "method not found");
        assert!(err.result_json.is_none());
        assert_eq!(err.error.as_ref().map(|e| e.code), Some(-32601));
    }
}
