//! Encryption utilities for NCM API
//!
//! Implements weapi, eapi, and linuxapi encryption schemes.

use aes::Aes128;
use aes::cipher::{BlockEncryptMut, KeyInit, KeyIvInit, block_padding::Pkcs7};
use base64::{Engine as _, engine::general_purpose};
use lazy_static::lazy_static;
use rsa::BigUint;
use urlencoding;

type Aes128CbcEnc = cbc::Encryptor<Aes128>;
type Aes128EcbEnc = ecb::Encryptor<Aes128>;

use AesMode::{Cbc, Ecb};

lazy_static! {
    static ref IV: Vec<u8> = "0102030405060708".as_bytes().to_vec();
    static ref PRESET_KEY: Vec<u8> = "0CoJUm6Qyw8W8jud".as_bytes().to_vec();
    static ref LINUX_API_KEY: Vec<u8> = "rFgB&h#%2?^eDg:Q".as_bytes().to_vec();
    static ref BASE62: Vec<u8> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".as_bytes().to_vec();
    // RSA public key modulus (n) and exponent (e) extracted from the PEM
    // Original PEM: MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDgtQn2JZ34ZC28NWYpAUd98iZ37BUrX/aKzmFbt7clFSs6sXqHauqKWqdtLkF2KexO40H1YTX8z2lSgBBOAxLsvaklV8k4cBFK9snQXE9/DDaFt6Rr7iVZMldczhC0JNgTz+SHXT6CBHuX3e9SdB1Ua44oncaTWz7OBGLbCiK45wIDAQAB
    static ref RSA_MODULUS: Vec<u8> = hex::decode("00e0b509f6259df8642dbc35662901477df22677ec152b5ff68ace615bb7b725152b3ab17a876aea8a5aa76d2e417629ec4ee341f56135fccf695280104e0312ecbda92557c93870114af6c9d05c4f7f0c3685b7a46bee255932575cce10b424d813cfe4875d3e82047b97ddef52741d546b8e289dc6935b3ece0462db0a22b8e7").unwrap();
    static ref RSA_EXPONENT: Vec<u8> = hex::decode("010001").unwrap();
    static ref EAPIKEY: Vec<u8> = "e82ckenh8dichen8".as_bytes().to_vec();
}

pub struct Crypto;

pub enum AesMode {
    Cbc,
    Ecb,
}

impl Crypto {
    pub fn eapi(url: &str, text: &str) -> String {
        let message = format!("nobody{}use{}md5forencrypt", url, text);
        let digest = hex::encode(md5::compute(message.as_bytes()).0);
        let data = format!("{}-36cd479b6b5-{}-36cd479b6b5-{}", url, text, digest);
        let params = Crypto::aes_encrypt(&data, &EAPIKEY, Ecb, Some(&*IV), |t: &Vec<u8>| {
            hex::encode_upper(t)
        });
        format!("params={}", urlencoding::encode(&params))
    }

    pub fn weapi(text: &str) -> String {
        let mut secret_key = [0u8; 16];
        use rand::Rng;
        rand::rng().fill(&mut secret_key[..]);
        let key: Vec<u8> = secret_key
            .iter()
            .map(|i| BASE62[(i % 62) as usize])
            .collect();

        let params1 = Crypto::aes_encrypt(text, &PRESET_KEY, Cbc, Some(&*IV), |t: &Vec<u8>| {
            general_purpose::STANDARD.encode(t)
        });

        let params = Crypto::aes_encrypt(&params1, &key, Cbc, Some(&*IV), |t: &Vec<u8>| {
            general_purpose::STANDARD.encode(t)
        });

        let enc_sec_key = Crypto::rsa_encrypt(
            std::str::from_utf8(&key.iter().rev().copied().collect::<Vec<u8>>()).unwrap(),
        );

        format!(
            "params={}&encSecKey={}",
            urlencoding::encode(&params),
            urlencoding::encode(&enc_sec_key)
        )
    }

    pub fn linuxapi(text: &str) -> String {
        let params = Crypto::aes_encrypt(text, &LINUX_API_KEY, Ecb, None, |t: &Vec<u8>| {
            hex::encode(t)
        })
        .to_uppercase();
        format!("eparams={}", urlencoding::encode(&params))
    }

    pub fn aes_encrypt(
        data: &str,
        key: &[u8],
        mode: AesMode,
        iv: Option<&[u8]>,
        encode: fn(&Vec<u8>) -> String,
    ) -> String {
        let data_bytes = data.as_bytes();
        // Calculate buffer size with padding
        let block_size = 16;
        let padded_len = ((data_bytes.len() / block_size) + 1) * block_size;
        let mut buf = vec![0u8; padded_len];
        buf[..data_bytes.len()].copy_from_slice(data_bytes);

        let cipher_text = match mode {
            Cbc => {
                let iv = iv.unwrap_or(&IV);
                let cipher = Aes128CbcEnc::new(key.into(), iv.into());
                cipher
                    .encrypt_padded_mut::<Pkcs7>(&mut buf, data_bytes.len())
                    .unwrap()
                    .to_vec()
            }
            Ecb => {
                let cipher = Aes128EcbEnc::new(key.into());
                cipher
                    .encrypt_padded_mut::<Pkcs7>(&mut buf, data_bytes.len())
                    .unwrap()
                    .to_vec()
            }
        };

        encode(&cipher_text)
    }

    pub fn rsa_encrypt(data: &str) -> String {
        // Use pre-extracted RSA modulus and exponent
        let n = BigUint::from_bytes_be(&RSA_MODULUS);
        let e = BigUint::from_bytes_be(&RSA_EXPONENT);

        // Pad data to 128 bytes (1024 bits) with leading zeros
        let prefix = vec![0u8; 128 - data.len()];
        let padded_data = [&prefix[..], data.as_bytes()].concat();

        // Raw RSA encryption without padding (same as OpenSSL Padding::NONE)
        // m^e mod n
        let m = BigUint::from_bytes_be(&padded_data);
        let encrypted = m.modpow(&e, &n);

        // Convert to fixed-size 128-byte output
        let encrypted_bytes = encrypted.to_bytes_be();
        let mut result = vec![0u8; 128];
        let start = 128 - encrypted_bytes.len();
        result[start..].copy_from_slice(&encrypted_bytes);

        hex::encode(result)
    }
}
