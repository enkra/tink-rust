// Copyright 2020 The Tink-Rust Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
////////////////////////////////////////////////////////////////////////////////

use std::collections::HashSet;
use tink_core::{utils::wrap_err, Mac, TinkError};
use tink_proto::prost::Message;
use tink_tests::proto_encode;

#[test]
fn test_get_primitive_cmac_basic() {
    tink_mac::init();
    let km = tink_core::registry::get_key_manager(tink_tests::AES_CMAC_TYPE_URL)
        .expect("AES-CMAC key manager not found");
    let test_keys = gen_valid_cmac_keys();
    for test_key in test_keys {
        let serialized_key = proto_encode(&test_key);
        let p = km.primitive(&serialized_key).unwrap();
        assert!(validate_cmac_primitive(p, &test_key).is_ok());
    }
}

#[test]
fn test_get_primitive_cmac_with_invalid_input() {
    tink_mac::init();
    let km = tink_core::registry::get_key_manager(tink_tests::AES_CMAC_TYPE_URL)
        .expect("AES CMAC key manager not found");
    // invalid key
    let test_keys = gen_invalid_cmac_keys();
    for (i, serialized_key) in test_keys.iter().enumerate() {
        assert!(
            km.primitive(serialized_key).is_err(),
            "expect an error in test case {}",
            i
        );
    }
    // empty input
    assert!(
        km.primitive(&[]).is_err(),
        "expect an error when input is empty"
    );
}

#[test]
fn test_new_key_cmac_multiple_times() {
    tink_mac::init();
    let km = tink_core::registry::get_key_manager(tink_tests::AES_CMAC_TYPE_URL)
        .expect("AES CMAC key manager not found");
    let serialized_format = proto_encode(&tink_tests::new_aes_cmac_key_format(16));
    let mut keys = HashSet::new();
    let n_test = 26;
    for _i in 0..n_test {
        let serialized_key = km.new_key(&serialized_format).unwrap();
        keys.insert(hex::encode(serialized_key));

        let key_data = km.new_key_data(&serialized_format).unwrap();
        keys.insert(hex::encode(key_data.value));
    }
    assert_eq!(keys.len(), 2 * n_test, "key is repeated");
}

#[test]
fn test_new_key_cmac_basic() {
    tink_mac::init();
    let km = tink_core::registry::get_key_manager(tink_tests::AES_CMAC_TYPE_URL)
        .expect("AES CMAC key manager not found");
    let test_formats = gen_valid_cmac_key_formats();
    for (i, test_format) in test_formats.iter().enumerate() {
        let serialized_format = proto_encode(test_format);
        let serialized_key = km
            .new_key(&serialized_format)
            .unwrap_or_else(|e| panic!("unexpected error in test case {}: {:?}", i, e));
        let key = tink_proto::AesCmacKey::decode(serialized_key.as_ref()).unwrap();
        assert!(validate_cmac_key(test_format, &key).is_ok());
    }
}

#[test]
fn test_new_key_cmac_with_invalid_input() {
    tink_mac::init();
    let km = tink_core::registry::get_key_manager(tink_tests::AES_CMAC_TYPE_URL)
        .expect("AES CMAC key manager not found");
    // invalid key formats
    let test_formats = gen_invalid_cmac_key_formats();
    for (i, serialized_format) in test_formats.iter().enumerate() {
        assert!(
            km.new_key(serialized_format).is_err(),
            "expect an error in test case {}",
            i
        );
    }
    // empty input
    assert!(
        km.new_key(&[]).is_err(),
        "expect an error when input is empty"
    );
}

#[test]
fn test_new_key_data_cmac_basic() {
    tink_mac::init();
    let km = tink_core::registry::get_key_manager(tink_tests::AES_CMAC_TYPE_URL)
        .expect("AES CMAC key manager not found");
    let test_formats = gen_valid_cmac_key_formats();
    for (i, test_format) in test_formats.iter().enumerate() {
        let serialized_format = proto_encode(test_format);
        let key_data = km
            .new_key_data(&serialized_format)
            .unwrap_or_else(|e| panic!("unexpected error in test case {}: {:?}", i, e));
        assert_eq!(
            key_data.type_url,
            tink_tests::AES_CMAC_TYPE_URL,
            "incorrect type url in test case {}",
            i
        );
        assert_eq!(
            key_data.key_material_type,
            tink_proto::key_data::KeyMaterialType::Symmetric as i32,
            "incorrect key material type in test case {}",
            i
        );
        let key =
            tink_proto::AesCmacKey::decode(key_data.value.as_ref()).expect("invalid key value");
        validate_cmac_key(test_format, &key).expect("invalid key");
    }
}

#[test]
fn test_new_key_data_cmac_with_invalid_input() {
    tink_mac::init();
    let km = tink_core::registry::get_key_manager(tink_tests::AES_CMAC_TYPE_URL)
        .expect("AES CMAC key manager not found");

    // invalid key formats
    let test_formats = gen_invalid_cmac_key_formats();
    for (i, serialized_format) in test_formats.iter().enumerate() {
        assert!(
            km.new_key_data(serialized_format).is_err(),
            "expect an error in test case {}",
            i
        );
    }
    // empty input
    assert!(
        km.new_key_data(&[]).is_err(),
        "expect an error when input is empty"
    );
}

#[test]
fn test_does_support_cmac() {
    tink_mac::init();
    let km = tink_core::registry::get_key_manager(tink_tests::AES_CMAC_TYPE_URL)
        .expect("AES CMAC key manager not found");
    assert!(
        km.does_support(tink_tests::AES_CMAC_TYPE_URL),
        "AesCmacKeyManager must support {}",
        tink_tests::AES_CMAC_TYPE_URL
    );
    assert!(
        !km.does_support("some bad type"),
        "AesCmacKeyManager must support only {}",
        tink_tests::AES_CMAC_TYPE_URL
    );
}

#[test]
fn test_cmac_type_url() {
    tink_mac::init();
    let km = tink_core::registry::get_key_manager(tink_tests::AES_CMAC_TYPE_URL)
        .expect("AES CMAC key manager not found");
    assert_eq!(
        km.type_url(),
        tink_tests::AES_CMAC_TYPE_URL,
        "incorrect key_type()"
    );
    assert_eq!(
        km.key_material_type(),
        tink_proto::key_data::KeyMaterialType::Symmetric
    );
    assert!(!km.supports_private_keys());
}

fn gen_invalid_cmac_keys() -> Vec<Vec<u8>> {
    let mut bad_version_key = tink_tests::new_aes_cmac_key(16);
    bad_version_key.version += 1;
    let mut short_key = tink_tests::new_aes_cmac_key(16);
    short_key.key_value = vec![1, 1];
    let non_key = tink_tests::new_aes_cmac_params(16);

    vec![
        proto_encode(&non_key),
        proto_encode(&bad_version_key),
        proto_encode(&tink_tests::new_aes_cmac_key(17)),
        proto_encode(&tink_tests::new_aes_cmac_key(1)),
        proto_encode(&short_key),
    ]
}

fn gen_invalid_cmac_key_formats() -> Vec<Vec<u8>> {
    let mut short_key_format = tink_tests::new_aes_cmac_key_format(16);
    short_key_format.key_size = 1;

    vec![
        // not a `AesCmacKeyFormat`
        proto_encode(&tink_tests::new_aes_cmac_params(16)),
        // tag size too big
        proto_encode(&tink_tests::new_aes_cmac_key_format(17)),
        // tag size too small
        proto_encode(&tink_tests::new_aes_cmac_key_format(1)),
        // key too short
        proto_encode(&short_key_format),
    ]
}

fn gen_valid_cmac_key_formats() -> Vec<tink_proto::AesCmacKeyFormat> {
    vec![
        tink_tests::new_aes_cmac_key_format(10),
        tink_tests::new_aes_cmac_key_format(16),
    ]
}

fn gen_valid_cmac_keys() -> Vec<tink_proto::AesCmacKey> {
    vec![
        tink_tests::new_aes_cmac_key(10),
        tink_tests::new_aes_cmac_key(16),
    ]
}

/// Check whether the given `AesCmacKey` matches the given key `AesCmacKeyFormat`
fn validate_cmac_key(
    format: &tink_proto::AesCmacKeyFormat,
    key: &tink_proto::AesCmacKey,
) -> Result<(), TinkError> {
    if format.key_size as usize != key.key_value.len()
        || key.params.as_ref().unwrap().tag_size != format.params.as_ref().unwrap().tag_size
    {
        return Err("key format and generated key do not match".into());
    }
    let p = tink_mac::subtle::AesCmac::new(
        &key.key_value,
        key.params.as_ref().unwrap().tag_size as usize,
    )
    .map_err(|e| wrap_err("cannot create primitive from key", e))?;
    validate_cmac_primitive(tink_core::Primitive::Mac(Box::new(p)), key)
}

/// Check whether the given primitive matches the given `AesCmacKey`.
fn validate_cmac_primitive(
    p: tink_core::Primitive,
    key: &tink_proto::AesCmacKey,
) -> Result<(), TinkError> {
    let cmac_primitive = match p {
        tink_core::Primitive::Mac(mac) => mac,
        _ => return Err("not a Mac primitive".into()),
    };
    let key_primitive = tink_mac::subtle::AesCmac::new(
        &key.key_value,
        key.params.as_ref().unwrap().tag_size as usize,
    )
    .map_err(|e| {
        wrap_err(
            &format!(
                "Could not create AES CMAC with key material {} and tag size {}",
                hex::encode(&key.key_value),
                key.params.as_ref().unwrap().tag_size
            ),
            e,
        )
    })?;

    let data = tink_core::subtle::random::get_random_bytes(20);
    let mac = cmac_primitive
        .compute_mac(&data)
        .map_err(|e| wrap_err("mac computation failed", e))?;
    let key_mac = key_primitive
        .compute_mac(&data)
        .map_err(|e| wrap_err("mac computation failed", e))?;
    cmac_primitive
        .verify_mac(&mac, &data)
        .expect("mac self verification failed");
    cmac_primitive
        .verify_mac(&key_mac, &data)
        .expect("mac computed with the provided key could not be verified");
    key_primitive
        .verify_mac(&mac, &data)
        .expect("mac could not be verified by primitive using the provided key");
    key_primitive
        .verify_mac(&key_mac, &data)
        .expect("mac self verification of mac created with the provided key failed");

    Ok(())
}
