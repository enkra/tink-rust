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

//! Provides common methods needed in test code.

#![deny(broken_intra_doc_links)]

use generic_array::typenum::Unsigned;
use p256::elliptic_curve;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use tink_core::{subtle::random::get_random_bytes, utils::wrap_err, Aead, TinkError};
use tink_proto::{prost, EcdsaSignatureEncoding, EllipticCurveType, HashType, KeyData, Keyset};

mod constant;
pub use constant::*;
pub mod fakekms;
mod sharedbuf;
pub use sharedbuf::*;
mod testdata;
pub use testdata::*;
mod wycheproofutil;
pub use wycheproofutil::*;

/// The [upstream Tink](https://github.com/google/tink) version that this Rust
/// port is based on.
pub const UPSTREAM_VERSION: &str = "1.6.0";

/// Dummy implementation of the `KeyManager` trait.
/// It returns [`DummyAead`] when `primitive()` functions are called.
#[derive(Debug)]
pub struct DummyAeadKeyManager {
    pub type_url: &'static str,
}

impl Default for DummyAeadKeyManager {
    fn default() -> Self {
        Self {
            type_url: AES_GCM_TYPE_URL,
        }
    }
}

impl tink_core::registry::KeyManager for DummyAeadKeyManager {
    fn primitive(&self, _serialized_key: &[u8]) -> Result<tink_core::Primitive, TinkError> {
        Ok(tink_core::Primitive::Aead(Box::new(DummyAead::default())))
    }

    fn new_key(&self, _serialized_key_format: &[u8]) -> Result<Vec<u8>, TinkError> {
        Err("not implemented".into())
    }

    fn type_url(&self) -> &'static str {
        self.type_url
    }

    fn key_material_type(&self) -> tink_proto::key_data::KeyMaterialType {
        tink_proto::key_data::KeyMaterialType::Symmetric
    }

    fn new_key_data(&self, _serialized_key_format: &[u8]) -> Result<KeyData, TinkError> {
        Err("not implemented".into())
    }
}

/// Dummy implementation of [`tink_core::Aead`] trait. It "encrypts" data with a simple
/// serialization capturing the dummy name, plaintext, and additional data, and "decrypts" it by
/// reversing this and checking that the name and additional data match.
#[derive(Clone, Debug, Default)]
pub struct DummyAead {
    pub name: String,
}

#[derive(Deserialize, Serialize)]
struct DummyAeadData {
    name: String,
    plaintext: Vec<u8>,
    additional_data: Vec<u8>,
}

impl tink_core::Aead for DummyAead {
    fn encrypt(&self, plaintext: &[u8], additional_data: &[u8]) -> Result<Vec<u8>, TinkError> {
        serde_json::to_vec(&DummyAeadData {
            name: self.name.clone(),
            plaintext: plaintext.to_vec(),
            additional_data: additional_data.to_vec(),
        })
        .map_err(|e| wrap_err("dummy aead encrypt", e))
    }

    fn decrypt(&self, ciphertext: &[u8], additional_data: &[u8]) -> Result<Vec<u8>, TinkError> {
        let data: DummyAeadData = serde_json::from_slice(ciphertext)
            .map_err(|e| wrap_err("dummy aeaed decrypt: invalid data", e))?;
        if data.name != self.name || data.additional_data != additional_data {
            Err("dummy aead encrypt: name/additional data mismatch".into())
        } else {
            Ok(data.plaintext)
        }
    }
}

/// Dummy implementation of the [`tink_core::Signer`] trait.
#[derive(Clone)]
pub struct DummySigner {
    aead: DummyAead,
}

impl DummySigner {
    /// Create a new dummy signer with the specified name. The name is used to pair with
    /// [`DummyVerifier`].
    pub fn new(name: &str) -> DummySigner {
        DummySigner {
            aead: DummyAead {
                name: format!("dummy public key: {}", name),
            },
        }
    }
}

impl tink_core::Signer for DummySigner {
    fn sign(&self, data: &[u8]) -> Result<Vec<u8>, TinkError> {
        self.aead.encrypt(&[], data)
    }
}

/// Dummy implementation of the [`tink_core::Verifier`] interface.
#[derive(Clone)]
pub struct DummyVerifier {
    aead: DummyAead,
}

impl DummyVerifier {
    /// Create a new dummy verifier with the specified name. The
    /// name is used to pair with the [`DummySigner`].
    pub fn new(name: &str) -> DummyVerifier {
        DummyVerifier {
            aead: DummyAead {
                name: format!("dummy public key: {}", name),
            },
        }
    }
}

impl tink_core::Verifier for DummyVerifier {
    fn verify(&self, signature: &[u8], data: &[u8]) -> Result<(), TinkError> {
        self.aead.decrypt(signature, data).map(|_| ())
    }
}

/// Dummy implementation of [`tink_core::Mac`] trait.
#[derive(Clone, Debug)]
pub struct DummyMac {
    pub name: String,
}

impl tink_core::Mac for DummyMac {
    // Computes message authentication code (MAC) for `data`.
    fn compute_mac(&self, data: &[u8]) -> Result<Vec<u8>, TinkError> {
        let mut m = Vec::new();
        m.extend_from_slice(data);
        m.extend_from_slice(self.name.as_bytes());
        Ok(m)
    }

    // Verify whether `mac` is a correct authentication code (MAC) for `data`.
    fn verify_mac(&self, _mac: &[u8], _data: &[u8]) -> Result<(), TinkError> {
        Ok(())
    }
}

/// Dummy implementation of a [`tink_core::registry::KmsClient`].
pub struct DummyKmsClient;

impl tink_core::registry::KmsClient for DummyKmsClient {
    fn supported(&self, key_uri: &str) -> bool {
        key_uri == "dummy"
    }

    fn get_aead(&self, _key_uri: &str) -> Result<Box<dyn tink_core::Aead>, TinkError> {
        Ok(Box::new(DummyAead::default()))
    }
}

/// Create a new [`Keyset`] containing an [`AesGcmKey`](tink_proto::AesGcmKey).
pub fn new_test_aes_gcm_keyset(primary_output_prefix_type: tink_proto::OutputPrefixType) -> Keyset {
    new_test_keyset(|| new_aes_gcm_key_data(16), primary_output_prefix_type)
}

/// Create a new [`Keyset`] containing an [`AesGcmSivKey`](tink_proto::AesGcmSivKey).
pub fn new_test_aes_gcm_siv_keyset(
    primary_output_prefix_type: tink_proto::OutputPrefixType,
) -> Keyset {
    new_test_keyset(|| new_aes_gcm_siv_key_data(16), primary_output_prefix_type)
}

/// Create a new [`Keyset`] containing an [`AesSivKey`](tink_proto::AesSivKey).
pub fn new_test_aes_siv_keyset(primary_output_prefix_type: tink_proto::OutputPrefixType) -> Keyset {
    new_test_keyset(new_aes_siv_key_data, primary_output_prefix_type)
}

/// Create a new [`Keyset`] containing an [`HmacKey`](tink_proto::HmacKey).
pub fn new_test_hmac_keyset(
    tag_size: u32,
    primary_output_prefix_type: tink_proto::OutputPrefixType,
) -> Keyset {
    new_test_keyset(
        || new_hmac_key_data(HashType::Sha256, tag_size),
        primary_output_prefix_type,
    )
}

/// Create a new [`Keyset`] containing an [`AesGcmHkdfKey`](tink_proto::AesGcmHkdfStreamingKey).
pub fn new_test_aes_gcm_hkdf_keyset() -> Keyset {
    const KEY_SIZE: u32 = 16;
    const DERIVED_KEY_SIZE: u32 = 16;
    const CIPHERTEXT_SEGMENT_SIZE: u32 = 4096;
    new_test_keyset(
        || {
            new_aes_gcm_hkdf_key_data(
                KEY_SIZE,
                DERIVED_KEY_SIZE,
                HashType::Sha256,
                CIPHERTEXT_SEGMENT_SIZE,
            )
        },
        tink_proto::OutputPrefixType::Raw,
    )
}

/// Create a new test [`Keyset`], generating fresh [`KeyData`] for each key using the provided
/// `key_data_generator`.
pub fn new_test_keyset<T>(
    key_data_generator: T,
    primary_output_prefix_type: tink_proto::OutputPrefixType,
) -> Keyset
where
    T: Fn() -> KeyData,
{
    let primary_key = new_key(
        &key_data_generator(),
        tink_proto::KeyStatusType::Enabled,
        42,
        primary_output_prefix_type,
    );
    let raw_key = new_key(
        &key_data_generator(),
        tink_proto::KeyStatusType::Enabled,
        43,
        tink_proto::OutputPrefixType::Raw,
    );
    let legacy_key = new_key(
        &key_data_generator(),
        tink_proto::KeyStatusType::Enabled,
        44,
        tink_proto::OutputPrefixType::Legacy,
    );
    let tink_key = new_key(
        &key_data_generator(),
        tink_proto::KeyStatusType::Enabled,
        45,
        tink_proto::OutputPrefixType::Tink,
    );
    let crunchy_key = new_key(
        &key_data_generator(),
        tink_proto::KeyStatusType::Enabled,
        46,
        tink_proto::OutputPrefixType::Crunchy,
    );
    let primary_key_id = primary_key.key_id;
    let keys = vec![primary_key, raw_key, legacy_key, tink_key, crunchy_key];
    new_keyset(primary_key_id, keys)
}

/// Return a dummy key that doesn't contain actual key material.
pub fn new_dummy_key(
    key_id: tink_core::KeyId,
    status: tink_proto::KeyStatusType,
    output_prefix_type: tink_proto::OutputPrefixType,
) -> tink_proto::keyset::Key {
    tink_proto::keyset::Key {
        key_data: Some(KeyData::default()),
        status: status as i32,
        key_id,
        output_prefix_type: output_prefix_type as i32,
    }
}

/// Create an [`EcdsaParams`](tink_proto::EcdsaParams) with the specified parameters.
pub fn new_ecdsa_params(
    hash_type: HashType,
    curve: EllipticCurveType,
    encoding: EcdsaSignatureEncoding,
) -> tink_proto::EcdsaParams {
    tink_proto::EcdsaParams {
        hash_type: hash_type as i32,
        curve: curve as i32,
        encoding: encoding as i32,
    }
}

/// Create an [`EcdsaKeyFormat`](tink_proto::EcdsaKeyFormat) with the specified parameters.
pub fn new_ecdsa_key_format(params: &tink_proto::EcdsaParams) -> tink_proto::EcdsaKeyFormat {
    tink_proto::EcdsaKeyFormat {
        params: Some(params.clone()),
    }
}

/// Create an [`EcdsaPrivateKey`](tink_proto::EcdsaPrivateKey) with randomly generated key
/// material.
pub fn new_random_ecdsa_private_key(
    hash_type: HashType,
    curve: EllipticCurveType,
) -> tink_proto::EcdsaPrivateKey {
    let mut csprng = p256::elliptic_curve::rand_core::OsRng {};
    let (secret_key_data, pub_x, pub_y) = match curve {
        EllipticCurveType::NistP256 => {
            let sk = p256::ecdsa::SigningKey::random(&mut csprng);
            let pk = p256::ecdsa::VerifyingKey::from(&sk);
            let point_len = elliptic_curve::FieldSize::<p256::NistP256>::to_usize();
            let pk_point = pk.to_encoded_point(/* compress= */ false);
            let pk_data = pk_point.as_bytes();
            (
                sk.to_bytes().to_vec(),
                pk_data[1..point_len + 1].to_vec(),
                pk_data[point_len + 1..].to_vec(),
            )
        }
        /* TODO(#16): more ECDSA curves
        EllipticCurveType::NistP384 => {
            let sk = p384::SecretKey::generate(&mut csprng);
            let pk = p384::PublicKey::from_secret_key(&sk, /* compressed= */ false).unwrap();
            let point_len =
                        <p384::NistP384 as elliptic_curve::Curve>::ElementSize::to_usize();
            let pk_data = pk.as_bytes();
            (
                sk.as_bytes().to_vec(),
                pk_data[..point_len].to_vec(),
                pk_data[point_len..].to_vec(),
            )
        }
        */
        _ => panic!("unsupported curve {:?}", curve),
    };
    let params = new_ecdsa_params(hash_type, curve, EcdsaSignatureEncoding::Der);
    let pub_key = tink_proto::EcdsaPublicKey {
        version: ECDSA_SIGNER_KEY_VERSION,
        params: Some(params),
        x: pub_x,
        y: pub_y,
    };

    tink_proto::EcdsaPrivateKey {
        version: ECDSA_SIGNER_KEY_VERSION,
        public_key: Some(pub_key),
        key_value: secret_key_data,
    }
}

/// Create an [`EcdsaPublicKey`](tink_proto::EcdsaPublicKey) with randomly generated key material.
pub fn new_random_ecdsa_public_key(
    hash_type: HashType,
    curve: EllipticCurveType,
) -> tink_proto::EcdsaPublicKey {
    new_random_ecdsa_private_key(hash_type, curve)
        .public_key
        .unwrap()
}

/// Return the enum representations of each parameter in the given
/// [`EcdsaParams`](tink_proto::EcdsaParams).
pub fn get_ecdsa_params(
    params: &tink_proto::EcdsaParams,
) -> (HashType, EllipticCurveType, EcdsaSignatureEncoding) {
    (
        HashType::from_i32(params.hash_type).unwrap(),
        EllipticCurveType::from_i32(params.curve).unwrap(),
        EcdsaSignatureEncoding::from_i32(params.encoding).unwrap(),
    )
}

/// Create an [`Ed25519PrivateKey`](tink_proto::Ed25519PrivateKey) with randomly generated key
/// material.
pub fn new_ed25519_private_key() -> tink_proto::Ed25519PrivateKey {
    let mut csprng = rand::thread_rng();
    let keypair = ed25519_dalek::Keypair::generate(&mut csprng);

    let public_proto = tink_proto::Ed25519PublicKey {
        version: ED25519_SIGNER_KEY_VERSION,
        key_value: keypair.public.as_bytes().to_vec(),
    };
    tink_proto::Ed25519PrivateKey {
        version: ED25519_SIGNER_KEY_VERSION,
        public_key: Some(public_proto),
        key_value: keypair.secret.as_bytes().to_vec(),
    }
}

/// Create an [`Ed25519PublicKey`](tink_proto::Ed25519PublicKey) with randomly generated key
/// material.
pub fn new_ed25519_public_key() -> tink_proto::Ed25519PublicKey {
    new_ed25519_private_key().public_key.unwrap()
}

/// Create a [`KeyData`] containing a randomly generated [`AesSivKey`](tink_proto::AesSivKey).
fn new_aes_siv_key_data() -> tink_proto::KeyData {
    let key_value = get_random_bytes(tink_daead::subtle::AES_SIV_KEY_SIZE);
    let key = &tink_proto::AesSivKey {
        version: AES_SIV_KEY_VERSION,
        key_value,
    };
    let serialized_key = proto_encode(key);
    new_key_data(
        AES_SIV_TYPE_URL,
        &serialized_key,
        tink_proto::key_data::KeyMaterialType::Symmetric,
    )
}

/// Create a randomly generated [`AesGcmKey`](tink_proto::AesGcmKey).
pub fn new_aes_gcm_key(key_version: u32, key_size: u32) -> tink_proto::AesGcmKey {
    let key_value = get_random_bytes(key_size.try_into().unwrap());
    tink_proto::AesGcmKey {
        version: key_version,
        key_value,
    }
}

/// Create a [`KeyData`] containing a randomly generated [`AesGcmKey`](tink_proto::AesGcmKey).
pub fn new_aes_gcm_key_data(key_size: u32) -> KeyData {
    let key = new_aes_gcm_key(AES_GCM_KEY_VERSION, key_size);
    let serialized_key = proto_encode(&key);
    new_key_data(
        AES_GCM_TYPE_URL,
        &serialized_key,
        tink_proto::key_data::KeyMaterialType::Symmetric,
    )
}

/// Return a new [`AesGcmKeyFormat`](tink_proto::AesGcmKeyFormat).
pub fn new_aes_gcm_key_format(key_size: u32) -> tink_proto::AesGcmKeyFormat {
    tink_proto::AesGcmKeyFormat {
        key_size,
        version: AES_GCM_KEY_VERSION,
    }
}

/// Create a randomly generated [`AesGcmSivKey`](tink_proto::AesGcmSivKey).
pub fn new_aes_gcm_siv_key(key_version: u32, key_size: u32) -> tink_proto::AesGcmSivKey {
    let key_value = get_random_bytes(key_size.try_into().unwrap());
    tink_proto::AesGcmSivKey {
        version: key_version,
        key_value,
    }
}

/// Create a [`KeyData`] containing a randomly generated
/// [`AesGcmSivKey`](tink_proto::AesGcmSivKey).
pub fn new_aes_gcm_siv_key_data(key_size: u32) -> KeyData {
    let key = new_aes_gcm_siv_key(AES_GCM_SIV_KEY_VERSION, key_size);
    let serialized_key = proto_encode(&key);
    new_key_data(
        AES_GCM_SIV_TYPE_URL,
        &serialized_key,
        tink_proto::key_data::KeyMaterialType::Symmetric,
    )
}

/// Create an [`AesGcmSivKey`](tink_proto::AesGcmSivKey) with randomly generated key material.
pub fn new_serialized_aes_gcm_siv_key(key_size: u32) -> Vec<u8> {
    let key = new_aes_gcm_siv_key(AES_GCM_SIV_KEY_VERSION, key_size);
    proto_encode(&key)
}

/// Return a new [`AesGcmSivKeyFormat`](tink_proto::AesGcmSivKeyFormat).
pub fn new_aes_gcm_siv_key_format(key_size: u32) -> tink_proto::AesGcmSivKeyFormat {
    tink_proto::AesGcmSivKeyFormat {
        key_size,
        version: AES_GCM_SIV_KEY_VERSION,
    }
}

/// Create a randomly generated [`AesGcmHkdfStreamingKey`](tink_proto::AesGcmHkdfStreamingKey).
pub fn new_aes_gcm_hkdf_key(
    key_version: u32,
    key_size: u32,
    derived_key_size: u32,
    hkdf_hash_type: i32,
    ciphertext_segment_size: u32,
) -> tink_proto::AesGcmHkdfStreamingKey {
    let key_value = get_random_bytes(key_size.try_into().unwrap());
    tink_proto::AesGcmHkdfStreamingKey {
        version: key_version,
        key_value,
        params: Some(tink_proto::AesGcmHkdfStreamingParams {
            ciphertext_segment_size,
            derived_key_size,
            hkdf_hash_type,
        }),
    }
}

/// Create a [`KeyData`] containing a randomly generated
/// [`AesGcmHkdfStreamingKey`](tink_proto::AesGcmHkdfStreamingKey).
pub fn new_aes_gcm_hkdf_key_data(
    key_size: u32,
    derived_key_size: u32,
    hkdf_hash_type: HashType,
    ciphertext_segment_size: u32,
) -> KeyData {
    let key = new_aes_gcm_hkdf_key(
        AES_GCM_HKDF_KEY_VERSION,
        key_size,
        derived_key_size,
        hkdf_hash_type as i32,
        ciphertext_segment_size,
    );
    let serialized_key = proto_encode(&key);
    new_key_data(
        AES_GCM_HKDF_TYPE_URL,
        &serialized_key,
        tink_proto::key_data::KeyMaterialType::Symmetric,
    )
}

/// Return a new [`AesGcmHkdfStreamingKeyFormat`](tink_proto::AesGcmHkdfStreamingKeyFormat).
pub fn new_aes_gcm_hkdf_key_format(
    key_size: u32,
    derived_key_size: u32,
    hkdf_hash_type: i32,
    ciphertext_segment_size: u32,
) -> tink_proto::AesGcmHkdfStreamingKeyFormat {
    tink_proto::AesGcmHkdfStreamingKeyFormat {
        version: AES_GCM_HKDF_KEY_VERSION,
        key_size,
        params: Some(tink_proto::AesGcmHkdfStreamingParams {
            ciphertext_segment_size,
            derived_key_size,
            hkdf_hash_type,
        }),
    }
}

/// Create a randomly generated [`AesCtrHmacStreamingKey`](tink_proto::AesCtrHmacStreamingKey).
pub fn new_aes_ctr_hmac_key(
    key_version: u32,
    key_size: u32,
    hkdf_hash_type: HashType,
    derived_key_size: u32,
    hash_type: HashType,
    tag_size: u32,
    ciphertext_segment_size: u32,
) -> tink_proto::AesCtrHmacStreamingKey {
    let key_value = get_random_bytes(key_size.try_into().unwrap());
    tink_proto::AesCtrHmacStreamingKey {
        version: key_version,
        key_value,
        params: Some(tink_proto::AesCtrHmacStreamingParams {
            ciphertext_segment_size,
            derived_key_size,
            hkdf_hash_type: hkdf_hash_type as i32,
            hmac_params: Some(tink_proto::HmacParams {
                hash: hash_type as i32,
                tag_size,
            }),
        }),
    }
}

/// Return a new [`AesCtrHmacStreamingKeyFormat`](tink_proto::AesCtrHmacStreamingKeyFormat).
pub fn new_aes_ctr_hmac_key_format(
    key_size: u32,
    hkdf_hash_type: HashType,
    derived_key_size: u32,
    hash_type: HashType,
    tag_size: u32,
    ciphertext_segment_size: u32,
) -> tink_proto::AesCtrHmacStreamingKeyFormat {
    tink_proto::AesCtrHmacStreamingKeyFormat {
        version: AES_CTR_HMAC_AEAD_KEY_VERSION,
        key_size,
        params: Some(tink_proto::AesCtrHmacStreamingParams {
            ciphertext_segment_size,
            derived_key_size,
            hkdf_hash_type: hkdf_hash_type as i32,
            hmac_params: Some(tink_proto::HmacParams {
                hash: hash_type as i32,
                tag_size,
            }),
        }),
    }
}

/// Return a new [`HmacParams`](tink_proto::HmacParams).
pub fn new_hmac_params(hash_type: HashType, tag_size: u32) -> tink_proto::HmacParams {
    tink_proto::HmacParams {
        hash: hash_type as i32,
        tag_size,
    }
}

/// Create a new [`HmacKey`](tink_proto::HmacKey) with the specified parameters.
pub fn new_hmac_key(hash_type: HashType, tag_size: u32) -> tink_proto::HmacKey {
    let params = new_hmac_params(hash_type, tag_size);
    let key_value = get_random_bytes(20);
    tink_proto::HmacKey {
        version: HMAC_KEY_VERSION,
        params: Some(params),
        key_value,
    }
}

/// Create a new [`HmacKeyFormat`](tink_proto::HmacKeyFormat) with the specified parameters.
pub fn new_hmac_key_format(hash_type: HashType, tag_size: u32) -> tink_proto::HmacKeyFormat {
    let params = new_hmac_params(hash_type, tag_size);
    let key_size = 20u32;
    tink_proto::HmacKeyFormat {
        params: Some(params),
        key_size,
        version: HMAC_KEY_VERSION,
    }
}

/// Return a new [`AesCmacParams`](tink_proto::AesCmacParams).
pub fn new_aes_cmac_params(tag_size: u32) -> tink_proto::AesCmacParams {
    tink_proto::AesCmacParams { tag_size }
}

/// Create a new [`AesCmacKey`](tink_proto::AesCmacKey) with the specified parameters.
pub fn new_aes_cmac_key(tag_size: u32) -> tink_proto::AesCmacKey {
    let params = new_aes_cmac_params(tag_size);
    let key_value = get_random_bytes(32);
    tink_proto::AesCmacKey {
        version: AES_CMAC_KEY_VERSION,
        params: Some(params),
        key_value,
    }
}

/// Create a new [`AesCmacKeyFormat`](tink_proto::AesCmacKeyFormat) with the specified parameters.
pub fn new_aes_cmac_key_format(tag_size: u32) -> tink_proto::AesCmacKeyFormat {
    let params = new_aes_cmac_params(tag_size);
    let key_size = 32u32;
    tink_proto::AesCmacKeyFormat {
        params: Some(params),
        key_size,
    }
}

/// Return a new [`tink_core::keyset::Manager`] that contains a [`HmacKey`](tink_proto::HmacKey).
pub fn new_hmac_keyset_manager() -> tink_core::keyset::Manager {
    let mut ksm = tink_core::keyset::Manager::new();
    let kt = tink_mac::hmac_sha256_tag128_key_template();
    ksm.rotate(&kt).expect("cannot rotate keyset manager");
    ksm
}

/// Return a new [`KeyData`] that contains a [`HmacKey`](tink_proto::HmacKey).
pub fn new_hmac_key_data(hash_type: HashType, tag_size: u32) -> KeyData {
    let key = new_hmac_key(hash_type, tag_size);
    let serialized_key = proto_encode(&key);
    KeyData {
        type_url: HMAC_TYPE_URL.to_string(),
        value: serialized_key,
        key_material_type: tink_proto::key_data::KeyMaterialType::Symmetric as i32,
    }
}

/// Return a new [`HmacPrfParams`](tink_proto::HmacPrfParams).
pub fn new_hmac_prf_params(hash_type: HashType) -> tink_proto::HmacPrfParams {
    tink_proto::HmacPrfParams {
        hash: hash_type as i32,
    }
}

/// Create a new [`HmacPrfKey`](tink_proto::HmacPrfKey) with the specified parameters.
pub fn new_hmac_prf_key(hash_type: HashType) -> tink_proto::HmacPrfKey {
    let params = new_hmac_prf_params(hash_type);
    let key_value = get_random_bytes(32);
    tink_proto::HmacPrfKey {
        version: HMAC_PRF_KEY_VERSION,
        params: Some(params),
        key_value,
    }
}

/// Create a new [`HmacPrfKeyFormat`](tink_proto::HmacPrfKeyFormat) with the specified parameters.
pub fn new_hmac_prf_key_format(hash_type: HashType) -> tink_proto::HmacPrfKeyFormat {
    let params = new_hmac_prf_params(hash_type);
    let key_size = 32u32;
    tink_proto::HmacPrfKeyFormat {
        params: Some(params),
        key_size,
        version: HMAC_PRF_KEY_VERSION,
    }
}

/// Return a new [`HkdfPrfParams`](tink_proto::HkdfPrfParams).
pub fn new_hkdf_prf_params(hash_type: HashType, salt: &[u8]) -> tink_proto::HkdfPrfParams {
    tink_proto::HkdfPrfParams {
        hash: hash_type as i32,
        salt: salt.to_vec(),
    }
}

/// Create a new [`HkdfPrfKey`](tink_proto::HkdfPrfKey) with the specified parameters.
pub fn new_hkdf_prf_key(hash_type: HashType, salt: &[u8]) -> tink_proto::HkdfPrfKey {
    let params = new_hkdf_prf_params(hash_type, salt);
    let key_value = get_random_bytes(32);
    tink_proto::HkdfPrfKey {
        version: HKDF_PRF_KEY_VERSION,
        params: Some(params),
        key_value,
    }
}

/// Create a new [`HkdfPrfKeyFormat`](tink_proto::HkdfPrfKeyFormat) with the specified parameters.
pub fn new_hkdf_prf_key_format(hash_type: HashType, salt: &[u8]) -> tink_proto::HkdfPrfKeyFormat {
    let params = new_hkdf_prf_params(hash_type, salt);
    let key_size = 32u32;
    tink_proto::HkdfPrfKeyFormat {
        params: Some(params),
        key_size,
        version: HKDF_PRF_KEY_VERSION,
    }
}

/// Create a new [`AesCmacPrfKey`](tink_proto::AesCmacPrfKey) with the specified parameters.
pub fn new_aes_cmac_prf_key() -> tink_proto::AesCmacPrfKey {
    let key_value = get_random_bytes(32);
    tink_proto::AesCmacPrfKey {
        version: AES_CMAC_PRF_KEY_VERSION,
        key_value,
    }
}

/// Create a new [`AesCmacPrfKeyFormat`](tink_proto::AesCmacPrfKeyFormat) with the specified
/// parameters.
pub fn new_aes_cmac_prf_key_format() -> tink_proto::AesCmacPrfKeyFormat {
    let key_size = 32u32;
    tink_proto::AesCmacPrfKeyFormat {
        version: AES_CMAC_PRF_KEY_VERSION,
        key_size,
    }
}

/// Create a new [`KeyData`] with the specified parameters.
pub fn new_key_data(
    type_url: &str,
    value: &[u8],
    material_type: tink_proto::key_data::KeyMaterialType,
) -> KeyData {
    KeyData {
        type_url: type_url.to_string(),
        value: value.to_vec(),
        key_material_type: material_type as i32,
    }
}

/// Create a new [`Key`](tink_proto::keyset::Key) with the specified parameters.
pub fn new_key(
    key_data: &KeyData,
    status: tink_proto::KeyStatusType,
    key_id: tink_core::KeyId,
    prefix_type: tink_proto::OutputPrefixType,
) -> tink_proto::keyset::Key {
    tink_proto::keyset::Key {
        key_data: Some(key_data.clone()),
        status: status as i32,
        key_id,
        output_prefix_type: prefix_type as i32,
    }
}

/// Create a new [`Keyset`] with the specified parameters.
pub fn new_keyset(primary_key_id: tink_core::KeyId, keys: Vec<tink_proto::keyset::Key>) -> Keyset {
    Keyset {
        primary_key_id,
        key: keys,
    }
}

/// Generate different byte mutations for a given byte array.
pub fn generate_mutations(src: &[u8]) -> Vec<Vec<u8>> {
    let mut all = Vec::new();

    // Flip bits
    for i in 0..src.len() {
        for j in 0..8u8 {
            let mut n = src.to_vec();
            n[i] ^= 1 << j;
            all.push(n);
        }
    }

    // truncate bytes
    for i in 1..src.len() {
        all.push(src[i..].to_vec());
    }

    // append extra byte
    let mut m = src.to_vec();
    m.push(0);
    all.push(m);
    all
}

/// Use a z test on the given byte string, expecting all bits to be uniformly set with probability
/// 1/2. Returns non ok status if the z test fails by more than 10 standard deviations.
///
/// With less statistics jargon: This counts the number of bits set and expects the number to be
/// roughly half of the length of the string. The law of large numbers suggests that we can assume
/// that the longer the string is, the more accurate that estimate becomes for a random string. This
/// test is useful to detect things like strings that are entirely zero.
///
/// Note: By itself, this is a very weak test for randomness.
pub fn z_test_uniform_string(bytes: &[u8]) -> Result<(), tink_core::TinkError> {
    let expected = (bytes.len() as f64) * 8.0 / 2.0;
    let stddev = ((bytes.len() as f64) * 8.0 / 4.0).sqrt();
    let mut num_set_bits: i64 = 0;
    for b in bytes {
        // Counting the number of bits set in byte:
        let mut b = *b;
        while b != 0 {
            num_set_bits += 1;
            b = b & (b - 1);
        }
    }
    // Check that the number of bits is within 10 stddevs.
    if ((num_set_bits as f64) - expected).abs() < 10.0 * stddev {
        Ok(())
    } else {
        Err(format!(
                "Z test for uniformly distributed variable out of bounds; Actual number of set bits was {} expected was {}, 10 * standard deviation is 10 * {} = {}",
            num_set_bits, expected, stddev, 10.0*stddev).into())
    }
}

fn rotate(bytes: &[u8]) -> Vec<u8> {
    let mut result = vec![0u8; bytes.len()];
    for i in 0..bytes.len() {
        let prev = if i == 0 { bytes.len() } else { i };
        result[i] = (bytes[i] >> 1) | (bytes[prev - 1] << 7);
    }
    result
}

/// Test that the cross-correlation of two byte strings of equal length points to independent and
/// uniformly distributed strings. Returns non `Ok` status if the z test fails by more than 10
/// standard deviations.
///
/// With less statistics jargon: This xors two strings and then performs the z_test_uniform_string
/// on the result. If the two strings are independent and uniformly distributed, the xor'ed string
/// is as well. A cross correlation test will find whether two strings overlap more or less than it
/// would be expected.
///
/// Note: Having a correlation of zero is only a necessary but not sufficient condition for
/// independence.
pub fn z_test_crosscorrelation_uniform_strings(
    bytes1: &[u8],
    bytes2: &[u8],
) -> Result<(), TinkError> {
    if bytes1.len() != bytes2.len() {
        return Err("Strings are not of equal length".into());
    }
    let mut crossed = vec![0u8; bytes1.len()];
    for i in 0..bytes1.len() {
        crossed[i] = bytes1[i] ^ bytes2[i]
    }
    z_test_uniform_string(&crossed)
}

/// Test whether the autocorrelation of a string
/// points to the bits being independent and uniformly distributed.
/// Rotates the string in a cyclic fashion. Returns non ok status if the z test
/// fails by more than 10 standard deviations.
///
/// With less statistics jargon: This rotates the string bit by bit and performs
/// z_test_crosscorrelation_uniform_strings on each of the rotated strings and the
/// original. This will find self similarity of the input string, especially
/// periodic self similarity. For example, it is a decent test to find English
/// text (needs about 180 characters with the current settings).
///
/// Note: Having a correlation of zero is only a necessary but not sufficient
/// condition for independence.
pub fn z_test_autocorrelation_uniform_string(bytes: &[u8]) -> Result<(), TinkError> {
    let mut rotated = bytes.to_vec();
    let mut violations = Vec::new();
    for i in 1..(bytes.len() * 8) {
        rotated = rotate(&rotated);
        if z_test_crosscorrelation_uniform_strings(bytes, &rotated).is_err() {
            violations.push(i.to_string());
        }
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(TinkError::new(&format!(
            "Autocorrelation exceeded 10 standard deviation at {} indices: {}",
            violations.len(),
            violations.join(", ")
        )))
    }
}

/// Return a [`EciesAeadHkdfPublicKey`](tink_proto::EciesAeadHkdfPublicKey) with specified
/// parameters.
pub fn ecies_aead_hkdf_public_key(
    c: EllipticCurveType,
    ht: HashType,
    ptfmt: tink_proto::EcPointFormat,
    dek_t: tink_proto::KeyTemplate,
    x: &[u8],
    y: &[u8],
    salt: &[u8],
) -> tink_proto::EciesAeadHkdfPublicKey {
    tink_proto::EciesAeadHkdfPublicKey {
        version: 0,
        params: Some(tink_proto::EciesAeadHkdfParams {
            kem_params: Some(tink_proto::EciesHkdfKemParams {
                curve_type: c as i32,
                hkdf_hash_type: ht as i32,
                hkdf_salt: salt.to_vec(),
            }),
            dem_params: Some(tink_proto::EciesAeadDemParams {
                aead_dem: Some(dek_t),
            }),
            ec_point_format: ptfmt as i32,
        }),
        x: x.to_vec(),
        y: y.to_vec(),
    }
}

/// Return an [`EciesAeadHkdfPrivateKey`](tink_proto::EciesAeadHkdfPrivateKey) with specified
/// parameters
pub fn ecies_aead_hkdf_private_key(
    p: tink_proto::EciesAeadHkdfPublicKey,
    d: &[u8],
) -> tink_proto::EciesAeadHkdfPrivateKey {
    tink_proto::EciesAeadHkdfPrivateKey {
        version: 0,
        public_key: Some(p),
        key_value: d.to_vec(),
    }
}

/// Generate a new EC key pair and returns the private key proto.
pub fn generate_ecies_aead_hkdf_private_key(
    curve: EllipticCurveType,
    ht: HashType,
    pt_fmt: tink_proto::EcPointFormat,
    dek_t: tink_proto::KeyTemplate,
    salt: &[u8],
) -> Result<tink_proto::EciesAeadHkdfPrivateKey, TinkError> {
    let pvt = tink_hybrid::subtle::generate_ecdh_key_pair(curve)?;
    let (x, y) = pvt.public_key().x_y_bytes()?;
    let pub_key = ecies_aead_hkdf_public_key(curve, ht, pt_fmt, dek_t, &x, &y, salt);
    Ok(ecies_aead_hkdf_private_key(pub_key, &pvt.d_bytes()))
}

/// Convert a protocol buffer message to its serialized form.
pub fn proto_encode<T>(msg: &T) -> Vec<u8>
where
    T: prost::Message,
{
    let mut data = Vec::new();
    msg.encode(&mut data)
        .expect("failed to encode proto message");
    data
}

/// Check for an expected error.
pub fn expect_err<T, E: std::fmt::Debug>(result: Result<T, E>, err_msg: &str) {
    assert!(result.is_err(), "expected error containing '{}'", err_msg);
    let err = result.err();
    assert!(
        format!("{:?}", err).contains(err_msg),
        "unexpected error {:?}, doesn't contain '{}'",
        err,
        err_msg
    );
}

/// Check for an expected error in a particular test case.
pub fn expect_err_for_case<T, E: std::fmt::Debug>(result: Result<T, E>, err_msg: &str, name: &str) {
    assert!(
        result.is_err(),
        "{}: expected error containing '{}'",
        name,
        err_msg
    );
    let err = result.err();
    assert!(
        format!("{:?}", err).contains(err_msg),
        "{}: unexpected error {:?}, doesn't contain '{}'",
        name,
        err,
        err_msg
    );
}

/// An object that implements [`std::io::Read`] and [`std::io::Write`] by always failing.
pub struct IoFailure {}

impl std::io::Read for IoFailure {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "failure object",
        ))
    }
}

impl std::io::Write for IoFailure {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "failure object",
        ))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
