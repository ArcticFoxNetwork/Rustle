//! Encryption utilities for NCM API
//!
//! Implements weapi, eapi, and linuxapi encryption schemes.

use base64::{Engine as _, engine::general_purpose};
use lazy_static::lazy_static;
use openssl::hash::{MessageDigest, hash};
use openssl::rsa::{Padding, Rsa};
use openssl::symm::{Cipher, encrypt};
use urlencoding;

use AesMode::{Cbc, Ecb};

lazy_static! {
    static ref IV: Vec<u8> = "0102030405060708".as_bytes().to_vec();
    static ref PRESET_KEY: Vec<u8> = "0CoJUm6Qyw8W8jud".as_bytes().to_vec();
    static ref LINUX_API_KEY: Vec<u8> = "rFgB&h#%2?^eDg:Q".as_bytes().to_vec();
    static ref BASE62: Vec<u8> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".as_bytes().to_vec();
    static ref RSA_PUBLIC_KEY: Vec<u8> = "-----BEGIN PUBLIC KEY-----\nMIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDgtQn2JZ34ZC28NWYpAUd98iZ37BUrX/aKzmFbt7clFSs6sXqHauqKWqdtLkF2KexO40H1YTX8z2lSgBBOAxLsvaklV8k4cBFK9snQXE9/DDaFt6Rr7iVZMldczhC0JNgTz+SHXT6CBHuX3e9SdB1Ua44oncaTWz7OBGLbCiK45wIDAQAB\n-----END PUBLIC KEY-----".as_bytes().to_vec();
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
        let digest = hex::encode(hash(MessageDigest::md5(), message.as_bytes()).unwrap());
        let data = format!("{}-36cd479b6b5-{}-36cd479b6b5-{}", url, text, digest);
        let params = Crypto::aes_encrypt(&data, &EAPIKEY, Ecb, Some(&*IV), |t: &Vec<u8>| {
            hex::encode_upper(t)
        });
        format!("params={}", urlencoding::encode(&params))
    }

    pub fn weapi(text: &str) -> String {
        let mut secret_key = [0u8; 16];
        rand::fill(&mut secret_key[..]);
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
            &RSA_PUBLIC_KEY,
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
        let cipher = match mode {
            Cbc => Cipher::aes_128_cbc(),
            Ecb => Cipher::aes_128_ecb(),
        };
        let cipher_text = encrypt(cipher, key, iv, data.as_bytes()).unwrap();

        encode(&cipher_text)
    }

    pub fn rsa_encrypt(data: &str, key: &[u8]) -> String {
        let rsa = Rsa::public_key_from_pem(key).unwrap();

        let prefix = vec![0u8; 128 - data.len()];

        let data = [&prefix[..], data.as_bytes()].concat();

        let mut buf = vec![0; rsa.size() as usize];

        rsa.public_encrypt(&data, &mut buf, Padding::NONE).unwrap();

        hex::encode(buf)
    }
}
