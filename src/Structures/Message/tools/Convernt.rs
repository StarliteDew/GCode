use anyhow::{anyhow, Result};
use serde_json::{Map, Number, Value};
use super::Structures::JValueType;



/// 尝试将 serde_json::Value 转换为目标类型
/// 
/// # 转换规则
/// - Null: 任何值转为 Null
/// - Bool: 数字非零为true，字符串支持 "true"/"false"/"1"/"0"/"yes"/"no"，null为false，数组/对象非空为true
/// - Number: 字符串尝试解析为整数或浮点数，bool true=1 false=0，null=0
/// - String: 任何值都转为字符串表示
/// - Array: 数组保持不变，其他包装为单元素数组
/// - Object: 仅对象可通过，其他报错
pub fn try_convert(value: Value, target: JValueType) -> Result<Value> {
    match target {
        JValueType::Null => Ok(Value::Null),
        JValueType::Bool => convert_to_bool(value),
        JValueType::Number => convert_to_number(value),
        JValueType::String => convert_to_string(value),
        JValueType::Array => convert_to_array(value),
        JValueType::Object => convert_to_object(value),
    }
}

fn convert_to_bool(value: Value) -> Result<Value> {
    let b = match value {
        Value::Bool(b) => b,
        Value::Null => false,
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i != 0
            } else if let Some(f) = n.as_f64() {
                f != 0.0 && !f.is_nan()
            } else {
                false
            }
        }
        Value::String(s) => {
            let lower = s.trim().to_lowercase();
            match lower.as_str() {
                "true" | "1" | "yes" | "on" => true,
                "false" | "0" | "no" | "off" | "" => false,
                _ => return Err(anyhow!("无法将字符串 {:?} 转换为布尔值", s)),
            }
        }
        Value::Array(arr) => !arr.is_empty(),
        Value::Object(obj) => !obj.is_empty(),
    };
    Ok(Value::Bool(b))
}

fn convert_to_number(value: Value) -> Result<Value> {
    match value {
        Value::Number(n) => Ok(Value::Number(n)),
        Value::Bool(b) => {
            let n = if b { 1 } else { 0 };
            Ok(Value::Number(Number::from(n)))
        }
        Value::String(s) => {
            let trimmed = s.trim();
            // 先尝试整数
            if let Ok(i) = trimmed.parse::<i64>() {
                return Ok(Value::Number(Number::from(i)));
            }
            // 再尝试浮点数
            if let Ok(f) = trimmed.parse::<f64>() {
                if let Some(n) = Number::from_f64(f) {
                    return Ok(Value::Number(n));
                }
            }
            Err(anyhow!("无法将字符串 {:?} 解析为数字", s))
        }
        Value::Null => Ok(Value::Number(Number::from(0))),
        _ => Err(anyhow!("无法将 {:?} 转换为数字", value)),
    }
}

fn convert_to_string(value: Value) -> Result<Value> {
    match value {
        Value::String(s) => Ok(Value::String(s)),
        Value::Null => Ok(Value::String("null".to_string())),
        Value::Bool(b) => Ok(Value::String(b.to_string())),
        Value::Number(n) => Ok(Value::String(n.to_string())),
        other => Ok(Value::String(other.to_string())),
    }
}

fn convert_to_array(value: Value) -> Result<Value> {
    match value {
        Value::Array(arr) => Ok(Value::Array(arr)),
        other => Ok(Value::Array(vec![other])),
    }
}

fn convert_to_object(value: Value) -> Result<Value> {
    match value {
        Value::Object(obj) => Ok(Value::Object(obj)),
        other => Err(anyhow!("无法将 {:?} 转换为 Object", other)),
    }
}

// ==================== 使用示例 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_to_number() {
        let v = Value::String("123".to_string());
        let result = try_convert(v, JValueType::Number).unwrap();
        assert_eq!(result, Value::Number(Number::from(123)));
    }

    #[test]
    fn test_number_to_bool() {
        let v = Value::Number(Number::from(1));
        let result = try_convert(v, JValueType::Bool).unwrap();
        assert_eq!(result, Value::Bool(true));

        let v = Value::Number(Number::from(0));
        let result = try_convert(v, JValueType::Bool).unwrap();
        assert_eq!(result, Value::Bool(false));
    }

    #[test]
    fn test_bool_to_number() {
        let v = Value::Bool(true);
        let result = try_convert(v, JValueType::Number).unwrap();
        assert_eq!(result, Value::Number(Number::from(1)));
    }

    #[test]
    fn test_number_to_string() {
        let v = Value::Number(Number::from(42));
        let result = try_convert(v, JValueType::String).unwrap();
        assert_eq!(result, Value::String("42".to_string()));
    }

    #[test]
    fn test_scalar_to_array() {
        let v = Value::String("hello".to_string());
        let result = try_convert(v, JValueType::Array).unwrap();
        assert_eq!(result, Value::Array(vec![Value::String("hello".to_string())]));
    }
}