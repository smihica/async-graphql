use rust_decimal::Decimal;

use crate::{InputValueError, InputValueResult, Scalar, ScalarType, Value};

#[Scalar(internal)]
impl ScalarType for Decimal {
    fn parse(value: Value) -> InputValueResult<Self> {
        match &value {
            Value::String(string) => Decimal::from_str_radix(string, 10)
                .map_err(|_| InputValueError::expected_type(value)),
            _ => Err(InputValueError::expected_type(value)),
        }
    }
    fn to_value(&self) -> Value {
        Value::String(self.to_string())
    }
}
