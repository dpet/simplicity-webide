use std::num::NonZeroU32;
use std::sync::Arc;

use elements::hashes::{sha256, Hash};
use elements::secp256k1_zkp;
use elements::secp256k1_zkp::rand;
use hex_conservative::{DisplayHex, FromHex};
use leptos::{
    component, create_memo, create_rw_signal, ev, event_target_value, html, use_context, view,
    with, For, IntoView, Memo, NodeRef, RwSignal, Signal, SignalGet, SignalGetUntracked, SignalSet,
    SignalUpdate, SignalWith, View,
};
use simfony::{elements, simplicity};

use crate::components::copy_to_clipboard::CopyToClipboard;

#[derive(Copy, Clone, Debug)]
pub struct SigningKeys {
    pub key_offset: RwSignal<u32>,
    pub key_count: RwSignal<NonZeroU32>,
    pub secret_keys: Memo<Vec<secp256k1_zkp::Keypair>>,
    pub public_keys: Memo<Vec<secp256k1_zkp::XOnlyPublicKey>>,
}

impl Default for SigningKeys {
    fn default() -> Self {
        Self::new(rand::random(), NonZeroU32::MIN)
    }
}

impl SigningKeys {
    pub fn new(key_offset: u32, key_count: NonZeroU32) -> Self {
        let key_offset = create_rw_signal(key_offset);
        let key_count = create_rw_signal(key_count);
        let secret_keys = create_memo(move |_| {
            (0..key_count.get().get())
                .map(|index| new_key(key_offset.get(), index))
                .collect::<Vec<secp256k1_zkp::Keypair>>()
        });
        let public_keys = create_memo(move |_| {
            with!(|secret_keys| {
                secret_keys
                    .iter()
                    .map(|key| key.x_only_public_key().0)
                    .collect()
            })
        });
        Self {
            key_offset,
            key_count,
            secret_keys,
            public_keys,
        }
    }

    pub fn first_public_key(&self) -> secp256k1_zkp::XOnlyPublicKey {
        self.public_keys.get_untracked()[0]
    }

    pub fn push_key(&self) {
        self.key_count.update(|n| *n = n.saturating_add(1));
    }

    pub fn pop_key(&self) {
        let n = self.key_count.get().get();
        if let Some(n_minus_one) = NonZeroU32::new(n - 1) {
            self.key_count.set(n_minus_one);
        }
    }

    pub fn signatures(
        self,
        message: Signal<secp256k1_zkp::Message>,
    ) -> Memo<Vec<secp256k1_zkp::schnorr::Signature>> {
        let secret_keys = self.secret_keys;
        create_memo(move |_| {
            with!(|secret_keys| {
                secret_keys
                    .iter()
                    .map(|key| key.sign_schnorr(message.get()))
                    .collect()
            })
        })
    }
}

fn new_key(offset: u32, index: u32) -> secp256k1_zkp::Keypair {
    let mut secret_key_bytes = [0u8; 32];
    secret_key_bytes[24..28].copy_from_slice(&offset.to_be_bytes());
    secret_key_bytes[28..].copy_from_slice(&index.to_be_bytes());
    let secret_key = secp256k1_zkp::SecretKey::from_slice(&secret_key_bytes)
        .expect("secret key should be valid");
    let keypair = secp256k1_zkp::Keypair::from_secret_key(secp256k1_zkp::SECP256K1, &secret_key);
    match keypair.x_only_public_key().1 {
        secp256k1_zkp::Parity::Even => keypair,
        secp256k1_zkp::Parity::Odd => {
            let secret_key = secret_key.negate();
            secp256k1_zkp::Keypair::from_secret_key(secp256k1_zkp::SECP256K1, &secret_key)
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum SignedDataMode {
    SighashAll,
    ThirtyTwoBytes,
    HashPreimageBytes,
}

#[derive(Clone, Copy, Debug)]
pub struct SignedData {
    mode: RwSignal<SignedDataMode>,
    sighash_all: Signal<sha256::Hash>,
    thirty_two_bytes: RwSignal<[u8; 32]>,
    hash_preimage_bytes: RwSignal<Vec<u8>>,
}

impl SignedData {
    pub fn new(
        tx_env: Signal<simplicity::jet::elements::ElementsEnv<Arc<elements::Transaction>>>,
    ) -> Self {
        let sighash_all =
            Signal::derive(move || with!(|tx_env| { tx_env.c_tx_env().sighash_all() }));
        Self {
            mode: create_rw_signal(SignedDataMode::SighashAll),
            sighash_all,
            thirty_two_bytes: create_rw_signal([0; 32]),
            hash_preimage_bytes: create_rw_signal(vec![]),
        }
    }

    pub fn message(self) -> Signal<secp256k1_zkp::Message> {
        Signal::derive(move || match self.mode.get() {
            SignedDataMode::SighashAll => {
                secp256k1_zkp::Message::from_digest(self.sighash_all.get().to_byte_array())
            }
            SignedDataMode::ThirtyTwoBytes => {
                secp256k1_zkp::Message::from_digest(self.thirty_two_bytes.get())
            }
            SignedDataMode::HashPreimageBytes => self.hash_preimage_bytes.with(|bytes| {
                secp256k1_zkp::Message::from_digest(sha256::Hash::hash(bytes).to_byte_array())
            }),
        })
    }
}

#[component]
pub fn KeyStoreTab() -> impl IntoView {
    view! {
        <div class="tab-content key-store-tab">
            <CopyPublicKeysToClipboard />
            <CopySignaturesToClipboard />
            <SelectSignedData />
        </div>
    }
}

#[component]
fn CopyPublicKeysToClipboard() -> impl IntoView {
    let signing_keys = use_context::<SigningKeys>().expect("signing keys should exist in context");
    let copy_single_public_key =
        move |(index, key): (usize, secp256k1_zkp::XOnlyPublicKey)| -> View {
            let label = format!("Key {}", index);
            let xonly_hex = move || format!("0x{}", key.serialize().as_hex());

            view! {
                <CopyToClipboard content=xonly_hex class="copy-button">
                    {label}
                    <i class="far fa-copy"></i>
                </CopyToClipboard>
            }
        };

    view! {
        <div>
            <p class="tab-description">
                "The keys and signatures that are currently generated by the web IDE are insecure."
            </p>
            <h3 class="tab-title">
                Public Keys
            </h3>
            <div class="button-row is-small">
                <For
                    each=move || signing_keys.public_keys.get().into_iter().enumerate()
                    key=|(_index, key)| *key
                    children=copy_single_public_key
                />
                <button
                    class="flat-button bordered"
                    type="button"
                    on:click=move |_| signing_keys.push_key()
                >
                    <i class="fas fa-plus"></i>
                    More
                </button>
                <button
                    class="flat-button bordered"
                    type="button"
                    on:click=move |_| signing_keys.pop_key()
                >
                    <i class="fas fa-minus"></i>
                    Less
                </button>
            </div>
        </div>
    }
}

#[component]
fn CopySignaturesToClipboard() -> impl IntoView {
    let signing_keys = use_context::<SigningKeys>().expect("signing keys should exist in context");
    let signed_data = use_context::<SignedData>().expect("signed data should exist in context");

    let copy_single_signature =
        move |(index, signature): (usize, secp256k1_zkp::schnorr::Signature)| -> View {
            let label = format!("Sig {}", index);
            let signature_hex = move || format!("0x{}", signature.serialize().as_hex());

            view! {
                <CopyToClipboard content=signature_hex class="copy-button">
                    {label}
                    <i class="far fa-copy"></i>
                </CopyToClipboard>
            }
        };

    view! {
        <div>
            <h3 class="tab-title">
                Signatures
            </h3>
            <div class="button-row is-small">
                <For
                    each=move || signing_keys.signatures(signed_data.message()).get().into_iter().enumerate()
                    key=|(_index, signature)| *signature
                    children=copy_single_signature
                />
                <button
                    class="flat-button bordered"
                    type="button"
                    on:click=move |_| signing_keys.push_key()
                >
                    <i class="fas fa-plus"></i>
                    More
                </button>
                <button
                    class="flat-button bordered"
                    type="button"
                    on:click=move |_| signing_keys.pop_key()
                >
                    <i class="fas fa-minus"></i>
                    Less
                </button>
            </div>
        </div>
    }
}

#[component]
fn SelectSignedData() -> impl IntoView {
    let signed_data = use_context::<SignedData>().expect("signed data should exist in context");
    let thirty_two_bytes_is_insane = create_rw_signal(false);
    let hash_preimage_bytes_is_insane = create_rw_signal(false);

    let sighash_all_initial_checked =
        matches!(signed_data.mode.get_untracked(), SignedDataMode::SighashAll);
    let thirty_two_bytes_initial_checked = matches!(
        signed_data.mode.get_untracked(),
        SignedDataMode::ThirtyTwoBytes
    );
    let hash_preimage_bytes_initial_checked = matches!(
        signed_data.mode.get_untracked(),
        SignedDataMode::HashPreimageBytes
    );
    let thirty_two_bytes_initial_value = format!(
        "0x{}",
        signed_data.thirty_two_bytes.get_untracked().as_hex()
    );
    let hash_preimage_bytes_initial_value = format!(
        "0x{}",
        signed_data.hash_preimage_bytes.get_untracked().as_hex()
    );

    let sighash_all_radio_ref = NodeRef::<html::Input>::new();
    let thirty_two_bytes_text_ref = NodeRef::<html::Input>::new();
    let hash_preimage_bytes_text_ref = NodeRef::<html::Input>::new();

    let select_sighash_all = move |_event: ev::Event| {
        signed_data.mode.set(SignedDataMode::SighashAll);
    };
    let select_thirty_two_bytes = move |_event: ev::Event| {
        signed_data.mode.set(SignedDataMode::ThirtyTwoBytes);
    };
    let select_hash_preimage_bytes = move |_event: ev::Event| {
        signed_data.mode.set(SignedDataMode::HashPreimageBytes);
    };
    let update_thirty_two_bytes = move |event: ev::Event| match <[u8; 32]>::from_hex(
        event_target_value(&event)
            .as_str()
            .trim()
            .trim_start_matches("0x"),
    ) {
        Ok(bytes) => {
            signed_data.thirty_two_bytes.set(bytes);
            thirty_two_bytes_text_ref
                .get()
                .expect("<input> should be mounted")
                .set_custom_validity("");
            thirty_two_bytes_is_insane.set(false);
        }
        Err(..) => {
            sighash_all_radio_ref
                .get()
                .expect("<input> should be mounted")
                .set_checked(true);
            thirty_two_bytes_text_ref
                .get()
                .expect("<input> should be mounted")
                .set_custom_validity("Expected exactly 64 hex digits");
            thirty_two_bytes_is_insane.set(true);
        }
    };
    let update_hash_preimage_bytes = move |event: ev::Event| match <Vec<u8>>::from_hex(
        event_target_value(&event)
            .as_str()
            .trim()
            .trim_start_matches("0x"),
    ) {
        Ok(bytes) => {
            signed_data.hash_preimage_bytes.set(bytes);
            hash_preimage_bytes_text_ref
                .get()
                .expect("<input> should be mounted")
                .set_custom_validity("");
            hash_preimage_bytes_is_insane.set(false);
        }
        Err(..) => {
            sighash_all_radio_ref
                .get()
                .expect("<input> should be mounted")
                .set_checked(true);
            hash_preimage_bytes_text_ref
                .get()
                .expect("<input> should be mounted")
                .set_custom_validity("Expected even number of hex digits");
            hash_preimage_bytes_is_insane.set(true);
        }
    };

    view! {
        <div>
            <h3 class="tab-title">
                Signed Data
            </h3>
            <fieldset class="button-col signed-data-content">
                <label class="key-store-display-row">
                    <input
                        type="radio"
                        name="signed_data"
                        checked=sighash_all_initial_checked
                        on:change=select_sighash_all
                        node_ref=sighash_all_radio_ref
                    />
                    <div class="display-row-label">
                        SIGHASH_ALL
                    </div>
                </label>
                <label class="key-store-display-row">
                    <input
                        type="radio"
                        name="signed_data"
                        checked=thirty_two_bytes_initial_checked
                        on:change=select_thirty_two_bytes
                        disabled=thirty_two_bytes_is_insane
                    />
                    <div class="display-row-label">
                        raw bytes
                    </div>
                    <input
                        class="input"
                        type="text"
                        placeholder="Enter hex bytes"
                        on:input=update_thirty_two_bytes
                        value=thirty_two_bytes_initial_value
                        node_ref=thirty_two_bytes_text_ref
                    />
                </label>
                <label class="key-store-display-row">
                    <input
                        type="radio"
                        name="signed_data"
                        checked=hash_preimage_bytes_initial_checked
                        on:change=select_hash_preimage_bytes
                        disabled=hash_preimage_bytes_is_insane
                    />
                    <div class="display-row-label">
                        byte hash
                    </div>
                    <input
                        class="input"
                        type="text"
                        placeholder="Enter hex bytes"
                        on:input=update_hash_preimage_bytes
                        value=hash_preimage_bytes_initial_value
                        node_ref=hash_preimage_bytes_text_ref
                    />
                </label>
            </fieldset>
        </div>
    }
}
