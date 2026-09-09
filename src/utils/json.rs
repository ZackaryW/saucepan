use serde::Serialize;

/// Encode compact JSON with recursively sorted object keys and unchanged array order.
///
/// Uses serde_json's value/number rules; this is not RFC 8785 canonicalization.
pub fn sorted_json<T: Serialize + ?Sized>(value: &T) -> serde_json::Result<Vec<u8>> {
    let mut value = serde_json::to_value(value)?;
    value.sort_all_objects();
    serde_json::to_vec(&value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::ser::{SerializeMap, Serializer};

    struct ReverseKeys;
    impl Serialize for ReverseKeys {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let mut map = serializer.serialize_map(Some(2))?;
            map.serialize_entry("z", &1)?;
            map.serialize_entry("a", &2)?;
            map.end()
        }
    }

    #[test]
    fn sorts_keys_recursively_including_objects_inside_arrays() {
        assert_eq!(
            sorted_json(&vec![ReverseKeys]).unwrap(),
            br#"[{"a":2,"z":1}]"#
        );
        assert_eq!(
            sorted_json(&serde_json::json!({"z": {"b": 1, "a": 2}, "a": 3})).unwrap(),
            br#"{"a":3,"z":{"a":2,"b":1}}"#
        );
    }

    #[test]
    fn preserves_array_order_types_and_utf8() {
        let value = serde_json::json!([3, "é\n", true, null, 1]);
        assert_eq!(
            sorted_json(&value).unwrap(),
            "[3,\"é\\n\",true,null,1]".as_bytes()
        );
    }

    #[test]
    fn accepts_unsized_serializable_inputs() {
        assert_eq!(sorted_json("hello").unwrap(), br#""hello""#);
        assert_eq!(sorted_json(&[2, 1][..]).unwrap(), b"[2,1]");
    }

    #[test]
    fn preserves_serializer_errors() {
        struct Broken;
        impl Serialize for Broken {
            fn serialize<S: Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
                Err(serde::ser::Error::custom("cannot serialize"))
            }
        }
        assert_eq!(
            sorted_json(&Broken).unwrap_err().to_string(),
            "cannot serialize"
        );
    }
}
