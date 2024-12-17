#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2021::*;
#[macro_use]
extern crate std;
use anchor_lang::prelude::*;
/// Generated external program declaration of program `order_engine`.
///
pub mod order_engine {
    use anchor_lang::prelude::*;
    use accounts::*;
    use events::*;
    use types::*;
    ///Program ID of program `order_engine`.
    pub static ID: Pubkey = __ID;
    /// Const version of `ID`
    pub const ID_CONST: Pubkey = __ID_CONST;
    /// The name is intentionally prefixed with `__` in order to reduce to possibility of name
    /// clashes with the crate's `ID`.
    static __ID: Pubkey = Pubkey::new_from_array([
        6u8,
        79u8,
        97u8,
        242u8,
        156u8,
        167u8,
        86u8,
        90u8,
        194u8,
        139u8,
        8u8,
        7u8,
        160u8,
        94u8,
        13u8,
        203u8,
        3u8,
        19u8,
        50u8,
        180u8,
        237u8,
        109u8,
        175u8,
        205u8,
        30u8,
        250u8,
        134u8,
        110u8,
        0u8,
        0u8,
        0u8,
        1u8,
    ]);
    const __ID_CONST: Pubkey = Pubkey::new_from_array([
        6u8,
        79u8,
        97u8,
        242u8,
        156u8,
        167u8,
        86u8,
        90u8,
        194u8,
        139u8,
        8u8,
        7u8,
        160u8,
        94u8,
        13u8,
        203u8,
        3u8,
        19u8,
        50u8,
        180u8,
        237u8,
        109u8,
        175u8,
        205u8,
        30u8,
        250u8,
        134u8,
        110u8,
        0u8,
        0u8,
        0u8,
        1u8,
    ]);
    /// Program definition.
    pub mod program {
        use super::*;
        /// Program type
        pub struct OrderEngine;
        #[automatically_derived]
        impl ::core::clone::Clone for OrderEngine {
            #[inline]
            fn clone(&self) -> OrderEngine {
                OrderEngine
            }
        }
        impl anchor_lang::Id for OrderEngine {
            fn id() -> Pubkey {
                super::__ID
            }
        }
    }
    /// Program constants.
    pub mod constants {
        pub const TEMPORARY_WSOL_TOKEN_ACCOUNT: &[u8] = &[
            116,
            101,
            109,
            112,
            111,
            114,
            97,
            114,
            121,
            45,
            119,
            115,
            111,
            108,
            45,
            116,
            111,
            107,
            101,
            110,
            45,
            97,
            99,
            99,
            111,
            117,
            110,
            116,
        ];
    }
    /// Program account type definitions.
    pub mod accounts {
        use super::*;
    }
    /// Program event type definitions.
    pub mod events {
        use super::*;
    }
    /// Program type definitions.
    ///
    /// Note that account and event type definitions are not included in this module, as they
    /// have their own dedicated modules.
    pub mod types {
        use super::*;
    }
    /// Cross program invocation (CPI) helpers.
    pub mod cpi {
        use super::*;
        pub fn fill<'a, 'b, 'c, 'info>(
            ctx: anchor_lang::context::CpiContext<
                'a,
                'b,
                'c,
                'info,
                accounts::Fill<'info>,
            >,
            input_amount: u64,
            output_amount: u64,
            expire_at: i64,
        ) -> anchor_lang::Result<()> {
            let ix = {
                let mut data = Vec::with_capacity(256);
                data.extend_from_slice(
                    &[168u8, 96u8, 183u8, 163u8, 92u8, 10u8, 40u8, 160u8],
                );
                AnchorSerialize::serialize(
                        &internal::args::Fill {
                            input_amount,
                            output_amount,
                            expire_at,
                        },
                        &mut data,
                    )
                    .map_err(|_| {
                        anchor_lang::error::ErrorCode::InstructionDidNotSerialize
                    })?;
                let accounts = ctx.to_account_metas(None);
                anchor_lang::solana_program::instruction::Instruction {
                    program_id: ctx.program.key(),
                    accounts,
                    data,
                }
            };
            let mut acc_infos = ctx.to_account_infos();
            anchor_lang::solana_program::program::invoke_signed(
                    &ix,
                    &acc_infos,
                    ctx.signer_seeds,
                )
                .map_or_else(|e| Err(Into::into(e)), |_| { Ok(()) })
        }
        pub struct Return<T> {
            phantom: std::marker::PhantomData<T>,
        }
        impl<T: AnchorDeserialize> Return<T> {
            pub fn get(&self) -> T {
                let (_key, data) = anchor_lang::solana_program::program::get_return_data()
                    .unwrap();
                T::try_from_slice(&data).unwrap()
            }
        }
        pub mod accounts {
            pub use super::internal::__cpi_client_accounts_fill::*;
        }
    }
    /// Off-chain client helpers.
    pub mod client {
        use super::*;
        /// Client args.
        pub mod args {
            pub use super::internal::args::*;
        }
        pub mod accounts {
            pub use super::internal::__client_accounts_fill::*;
        }
    }
    #[doc(hidden)]
    mod internal {
        use super::*;
        /// An Anchor generated module containing the program's set of instructions, where each
        /// method handler in the `#[program]` mod is associated with a struct defining the input
        /// arguments to the method. These should be used directly, when one wants to serialize
        /// Anchor instruction data, for example, when specifying instructions instructions on a
        /// client.
        pub mod args {
            use super::*;
            /// Instruction argument
            pub struct Fill {
                pub input_amount: u64,
                pub output_amount: u64,
                pub expire_at: i64,
            }
            impl borsh::ser::BorshSerialize for Fill
            where
                u64: borsh::ser::BorshSerialize,
                u64: borsh::ser::BorshSerialize,
                i64: borsh::ser::BorshSerialize,
            {
                fn serialize<W: borsh::maybestd::io::Write>(
                    &self,
                    writer: &mut W,
                ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
                    borsh::BorshSerialize::serialize(&self.input_amount, writer)?;
                    borsh::BorshSerialize::serialize(&self.output_amount, writer)?;
                    borsh::BorshSerialize::serialize(&self.expire_at, writer)?;
                    Ok(())
                }
            }
            impl borsh::de::BorshDeserialize for Fill
            where
                u64: borsh::BorshDeserialize,
                u64: borsh::BorshDeserialize,
                i64: borsh::BorshDeserialize,
            {
                fn deserialize_reader<R: borsh::maybestd::io::Read>(
                    reader: &mut R,
                ) -> ::core::result::Result<Self, borsh::maybestd::io::Error> {
                    Ok(Self {
                        input_amount: borsh::BorshDeserialize::deserialize_reader(
                            reader,
                        )?,
                        output_amount: borsh::BorshDeserialize::deserialize_reader(
                            reader,
                        )?,
                        expire_at: borsh::BorshDeserialize::deserialize_reader(reader)?,
                    })
                }
            }
            impl anchor_lang::Discriminator for Fill {
                const DISCRIMINATOR: &'static [u8] = &[
                    168u8,
                    96u8,
                    183u8,
                    163u8,
                    92u8,
                    10u8,
                    40u8,
                    160u8,
                ];
            }
            impl anchor_lang::InstructionData for Fill {}
            impl anchor_lang::Owner for Fill {
                fn owner() -> Pubkey {
                    super::__ID
                }
            }
        }
        /// An internal, Anchor generated module. This is used (as an
        /// implementation detail), to generate a CPI struct for a given
        /// `#[derive(Accounts)]` implementation, where each field is an
        /// AccountInfo.
        ///
        /// To access the struct in this module, one should use the sibling
        /// [`cpi::accounts`] module (also generated), which re-exports this.
        pub(crate) mod __cpi_client_accounts_fill {
            use super::*;
            /// Generated CPI struct of the accounts for [`Fill`].
            pub struct Fill<'info> {
                pub taker: anchor_lang::solana_program::account_info::AccountInfo<'info>,
                pub maker: anchor_lang::solana_program::account_info::AccountInfo<'info>,
                pub taker_input_mint_token_account: Option<
                    anchor_lang::solana_program::account_info::AccountInfo<'info>,
                >,
                pub maker_input_mint_token_account: Option<
                    anchor_lang::solana_program::account_info::AccountInfo<'info>,
                >,
                pub taker_output_mint_token_account: Option<
                    anchor_lang::solana_program::account_info::AccountInfo<'info>,
                >,
                pub maker_output_mint_token_account: Option<
                    anchor_lang::solana_program::account_info::AccountInfo<'info>,
                >,
                pub input_mint: anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >,
                pub input_token_program: anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >,
                pub output_mint: anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >,
                pub output_token_program: anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >,
                pub system_program: anchor_lang::solana_program::account_info::AccountInfo<
                    'info,
                >,
            }
            #[automatically_derived]
            impl<'info> anchor_lang::ToAccountMetas for Fill<'info> {
                fn to_account_metas(
                    &self,
                    is_signer: Option<bool>,
                ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                    let mut account_metas = ::alloc::vec::Vec::new();
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                anchor_lang::Key::key(&self.taker),
                                true,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                anchor_lang::Key::key(&self.maker),
                                true,
                            ),
                        );
                    if let Some(taker_input_mint_token_account) = &self
                        .taker_input_mint_token_account
                    {
                        account_metas
                            .push(
                                anchor_lang::solana_program::instruction::AccountMeta::new(
                                    anchor_lang::Key::key(taker_input_mint_token_account),
                                    false,
                                ),
                            );
                    } else {
                        account_metas
                            .push(
                                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                    super::__ID,
                                    false,
                                ),
                            );
                    }
                    if let Some(maker_input_mint_token_account) = &self
                        .maker_input_mint_token_account
                    {
                        account_metas
                            .push(
                                anchor_lang::solana_program::instruction::AccountMeta::new(
                                    anchor_lang::Key::key(maker_input_mint_token_account),
                                    false,
                                ),
                            );
                    } else {
                        account_metas
                            .push(
                                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                    super::__ID,
                                    false,
                                ),
                            );
                    }
                    if let Some(taker_output_mint_token_account) = &self
                        .taker_output_mint_token_account
                    {
                        account_metas
                            .push(
                                anchor_lang::solana_program::instruction::AccountMeta::new(
                                    anchor_lang::Key::key(taker_output_mint_token_account),
                                    false,
                                ),
                            );
                    } else {
                        account_metas
                            .push(
                                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                    super::__ID,
                                    false,
                                ),
                            );
                    }
                    if let Some(maker_output_mint_token_account) = &self
                        .maker_output_mint_token_account
                    {
                        account_metas
                            .push(
                                anchor_lang::solana_program::instruction::AccountMeta::new(
                                    anchor_lang::Key::key(maker_output_mint_token_account),
                                    false,
                                ),
                            );
                    } else {
                        account_metas
                            .push(
                                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                    super::__ID,
                                    false,
                                ),
                            );
                    }
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                anchor_lang::Key::key(&self.input_mint),
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                anchor_lang::Key::key(&self.input_token_program),
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                anchor_lang::Key::key(&self.output_mint),
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                anchor_lang::Key::key(&self.output_token_program),
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                anchor_lang::Key::key(&self.system_program),
                                false,
                            ),
                        );
                    account_metas
                }
            }
            #[automatically_derived]
            impl<'info> anchor_lang::ToAccountInfos<'info> for Fill<'info> {
                fn to_account_infos(
                    &self,
                ) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<'info>> {
                    let mut account_infos = ::alloc::vec::Vec::new();
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(&self.taker),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(&self.maker),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(
                                &self.taker_input_mint_token_account,
                            ),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(
                                &self.maker_input_mint_token_account,
                            ),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(
                                &self.taker_output_mint_token_account,
                            ),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(
                                &self.maker_output_mint_token_account,
                            ),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(
                                &self.input_mint,
                            ),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(
                                &self.input_token_program,
                            ),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(
                                &self.output_mint,
                            ),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(
                                &self.output_token_program,
                            ),
                        );
                    account_infos
                        .extend(
                            anchor_lang::ToAccountInfos::to_account_infos(
                                &self.system_program,
                            ),
                        );
                    account_infos
                }
            }
        }
        /// An internal, Anchor generated module. This is used (as an
        /// implementation detail), to generate a struct for a given
        /// `#[derive(Accounts)]` implementation, where each field is a Pubkey,
        /// instead of an `AccountInfo`. This is useful for clients that want
        /// to generate a list of accounts, without explicitly knowing the
        /// order all the fields should be in.
        ///
        /// To access the struct in this module, one should use the sibling
        /// `accounts` module (also generated), which re-exports this.
        pub(crate) mod __client_accounts_fill {
            use super::*;
            use anchor_lang::prelude::borsh;
            /// Generated client accounts for [`Fill`].
            pub struct Fill {
                pub taker: Pubkey,
                pub maker: Pubkey,
                pub taker_input_mint_token_account: Option<Pubkey>,
                pub maker_input_mint_token_account: Option<Pubkey>,
                pub taker_output_mint_token_account: Option<Pubkey>,
                pub maker_output_mint_token_account: Option<Pubkey>,
                pub input_mint: Pubkey,
                pub input_token_program: Pubkey,
                pub output_mint: Pubkey,
                pub output_token_program: Pubkey,
                pub system_program: Pubkey,
            }
            impl borsh::ser::BorshSerialize for Fill
            where
                Pubkey: borsh::ser::BorshSerialize,
                Pubkey: borsh::ser::BorshSerialize,
                Option<Pubkey>: borsh::ser::BorshSerialize,
                Option<Pubkey>: borsh::ser::BorshSerialize,
                Option<Pubkey>: borsh::ser::BorshSerialize,
                Option<Pubkey>: borsh::ser::BorshSerialize,
                Pubkey: borsh::ser::BorshSerialize,
                Pubkey: borsh::ser::BorshSerialize,
                Pubkey: borsh::ser::BorshSerialize,
                Pubkey: borsh::ser::BorshSerialize,
                Pubkey: borsh::ser::BorshSerialize,
            {
                fn serialize<W: borsh::maybestd::io::Write>(
                    &self,
                    writer: &mut W,
                ) -> ::core::result::Result<(), borsh::maybestd::io::Error> {
                    borsh::BorshSerialize::serialize(&self.taker, writer)?;
                    borsh::BorshSerialize::serialize(&self.maker, writer)?;
                    borsh::BorshSerialize::serialize(
                        &self.taker_input_mint_token_account,
                        writer,
                    )?;
                    borsh::BorshSerialize::serialize(
                        &self.maker_input_mint_token_account,
                        writer,
                    )?;
                    borsh::BorshSerialize::serialize(
                        &self.taker_output_mint_token_account,
                        writer,
                    )?;
                    borsh::BorshSerialize::serialize(
                        &self.maker_output_mint_token_account,
                        writer,
                    )?;
                    borsh::BorshSerialize::serialize(&self.input_mint, writer)?;
                    borsh::BorshSerialize::serialize(&self.input_token_program, writer)?;
                    borsh::BorshSerialize::serialize(&self.output_mint, writer)?;
                    borsh::BorshSerialize::serialize(
                        &self.output_token_program,
                        writer,
                    )?;
                    borsh::BorshSerialize::serialize(&self.system_program, writer)?;
                    Ok(())
                }
            }
            #[automatically_derived]
            impl anchor_lang::ToAccountMetas for Fill {
                fn to_account_metas(
                    &self,
                    is_signer: Option<bool>,
                ) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                    let mut account_metas = ::alloc::vec::Vec::new();
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                self.taker,
                                true,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new(
                                self.maker,
                                true,
                            ),
                        );
                    if let Some(taker_input_mint_token_account) = &self
                        .taker_input_mint_token_account
                    {
                        account_metas
                            .push(
                                anchor_lang::solana_program::instruction::AccountMeta::new(
                                    *taker_input_mint_token_account,
                                    false,
                                ),
                            );
                    } else {
                        account_metas
                            .push(
                                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                    super::__ID,
                                    false,
                                ),
                            );
                    }
                    if let Some(maker_input_mint_token_account) = &self
                        .maker_input_mint_token_account
                    {
                        account_metas
                            .push(
                                anchor_lang::solana_program::instruction::AccountMeta::new(
                                    *maker_input_mint_token_account,
                                    false,
                                ),
                            );
                    } else {
                        account_metas
                            .push(
                                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                    super::__ID,
                                    false,
                                ),
                            );
                    }
                    if let Some(taker_output_mint_token_account) = &self
                        .taker_output_mint_token_account
                    {
                        account_metas
                            .push(
                                anchor_lang::solana_program::instruction::AccountMeta::new(
                                    *taker_output_mint_token_account,
                                    false,
                                ),
                            );
                    } else {
                        account_metas
                            .push(
                                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                    super::__ID,
                                    false,
                                ),
                            );
                    }
                    if let Some(maker_output_mint_token_account) = &self
                        .maker_output_mint_token_account
                    {
                        account_metas
                            .push(
                                anchor_lang::solana_program::instruction::AccountMeta::new(
                                    *maker_output_mint_token_account,
                                    false,
                                ),
                            );
                    } else {
                        account_metas
                            .push(
                                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                    super::__ID,
                                    false,
                                ),
                            );
                    }
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                self.input_mint,
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                self.input_token_program,
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                self.output_mint,
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                self.output_token_program,
                                false,
                            ),
                        );
                    account_metas
                        .push(
                            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                                self.system_program,
                                false,
                            ),
                        );
                    account_metas
                }
            }
        }
    }
    /// Program utilities.
    pub mod utils {
        use super::*;
        /// An enum that includes all accounts of the declared program as a tuple variant.
        ///
        /// See [`Self::try_from_bytes`] to create an instance from bytes.
        pub enum Account {}
        impl Account {
            /// Try to create an account based on the given bytes.
            ///
            /// This method returns an error if the discriminator of the given bytes don't match
            /// with any of the existing accounts, or if the deserialization fails.
            pub fn try_from_bytes(bytes: &[u8]) -> Result<Self> {
                Self::try_from(bytes)
            }
        }
        impl TryFrom<&[u8]> for Account {
            type Error = anchor_lang::error::Error;
            fn try_from(value: &[u8]) -> Result<Self> {
                Err(ProgramError::InvalidArgument.into())
            }
        }
        /// An enum that includes all events of the declared program as a tuple variant.
        ///
        /// See [`Self::try_from_bytes`] to create an instance from bytes.
        pub enum Event {}
        impl Event {
            /// Try to create an event based on the given bytes.
            ///
            /// This method returns an error if the discriminator of the given bytes don't match
            /// with any of the existing events, or if the deserialization fails.
            pub fn try_from_bytes(bytes: &[u8]) -> Result<Self> {
                Self::try_from(bytes)
            }
        }
        impl TryFrom<&[u8]> for Event {
            type Error = anchor_lang::error::Error;
            fn try_from(value: &[u8]) -> Result<Self> {
                Err(ProgramError::InvalidArgument.into())
            }
        }
    }
}
pub mod fill {
    use crate::order_engine;
    use anchor_lang::{AnchorDeserialize, Discriminator};
    use anchor_spl::associated_token;
    use anyhow::{anyhow, bail, ensure, Context, Result};
    use solana_sdk::{
        borsh1::try_from_slice_unchecked,
        compute_budget::{self, ComputeBudgetInstruction},
        message::SanitizedMessage, pubkey::Pubkey,
        sysvar::instructions::BorrowedInstruction,
    };
    pub struct Order {
        pub taker: Pubkey,
        pub maker: Pubkey,
        pub in_amount: u64,
        pub input_mint: Pubkey,
        pub out_amount: u64,
        pub output_mint: Pubkey,
        pub expire_at: i64,
    }
    pub struct ValidatedFill {
        pub compute_unit_limit: u32,
        /// The maker should verify that the trade is still viable should the compute unit price change drastically
        /// The compute unit price might change from the original tx as wallets tend to mutate it
        pub compute_unit_price: u64,
    }
    /// Given the knowledge of the order, validate the fill transaction
    pub fn validate_fill_sanitized_message(
        sanitized_message: &SanitizedMessage,
        order: Order,
    ) -> Result<ValidatedFill> {
        let fee_payer = sanitized_message.fee_payer();
        if ::anyhow::__private::not(fee_payer == &order.maker) {
            return ::anyhow::__private::Err(
                ::anyhow::Error::msg({
                    let res = ::alloc::fmt::format(
                        format_args!(
                            "Fee payer was not the expected maker {1} but was {0}",
                            order.maker,
                            fee_payer,
                        ),
                    );
                    res
                }),
            );
        }
        if ::anyhow::__private::not(
            sanitized_message.get_signature_details().num_transaction_signatures() == 2,
        ) {
            return ::anyhow::__private::Err({
                let error = ::anyhow::__private::format_err(
                    format_args!("Too many signers"),
                );
                error
            });
        }
        let second_signer = sanitized_message
            .account_keys()
            .get(1)
            .context("Missing taker")?;
        if ::anyhow::__private::not(second_signer == &order.taker) {
            return ::anyhow::__private::Err({
                let error = ::anyhow::__private::format_err(
                    format_args!("Second transaction signer is not the taker"),
                );
                error
            });
        }
        let mut fill_ix_found = false;
        let mut compute_unit_limit = None;
        let mut compute_unit_price = None;
        for BorrowedInstruction { program_id, accounts, data } in sanitized_message
            .decompile_instructions()
        {
            if program_id == &compute_budget::ID {
                let compute_budget_ix = try_from_slice_unchecked::<
                    ComputeBudgetInstruction,
                >(data)?;
                match compute_budget_ix {
                    ComputeBudgetInstruction::SetComputeUnitLimit(limit) => {
                        compute_unit_limit = Some(limit);
                    }
                    ComputeBudgetInstruction::SetComputeUnitPrice(price) => {
                        if ::anyhow::__private::not(compute_unit_price.is_none()) {
                            return ::anyhow::__private::Err({
                                let error = ::anyhow::__private::format_err(
                                    format_args!("Compute unit price is already set"),
                                );
                                error
                            });
                        }
                        compute_unit_price = Some(price);
                    }
                    _ => {
                        return ::anyhow::__private::Err({
                            let error = ::anyhow::__private::format_err(
                                format_args!("Unexpected compute budget instruction"),
                            );
                            error
                        });
                    }
                }
            } else if program_id == &associated_token::ID {
                if ::anyhow::__private::not(
                    data == <[_]>::into_vec(#[rustc_box] ::alloc::boxed::Box::new([1])),
                ) {
                    return ::anyhow::__private::Err({
                        let error = ::anyhow::__private::format_err(
                            format_args!(
                                "Incorrect associated token account program data",
                            ),
                        );
                        error
                    });
                }
                match (&accounts.first().map(|am| am.pubkey), &Some(&order.taker)) {
                    (lhs, rhs) => {
                        if !(lhs == rhs) {
                            #[allow(unused_imports)]
                            use ::anyhow::__private::{BothDebug, NotBothDebug};
                            return Err(
                                (lhs, rhs)
                                    .__dispatch_ensure(
                                        "Condition failed: `accounts.first().map(|am| am.pubkey) == Some(&order.taker)`",
                                    ),
                            );
                        }
                    }
                };
            } else if program_id == &order_engine::ID {
                if ::anyhow::__private::not(!fill_ix_found) {
                    return ::anyhow::__private::Err({
                        let error = ::anyhow::__private::format_err(
                            format_args!("Duplicated fill instruction"),
                        );
                        error
                    });
                }
                fill_ix_found = true;
                if ::anyhow::__private::not(data.len() >= 8) {
                    return ::anyhow::__private::Err({
                        let error = ::anyhow::__private::format_err(
                            format_args!("Not enough data in fill instruction"),
                        );
                        error
                    });
                }
                let (discriminator, ix_data) = data.split_at(8);
                if ::anyhow::__private::not(
                    discriminator == order_engine::client::args::Fill::DISCRIMINATOR,
                ) {
                    return ::anyhow::__private::Err({
                        let error = ::anyhow::__private::format_err(
                            format_args!("Not a fill discriminator"),
                        );
                        error
                    });
                }
                let pubkeys = accounts
                    .into_iter()
                    .map(|a| *a.pubkey)
                    .collect::<Vec<_>>();
                let [taker, maker, _taker_input_mint_token_account,
                _maker_input_mint_token_account, _taker_output_mint_token_account,
                _maker_output_mint_token_account, input_mint, _input_token_program,
                output_mint, _output_mint_token_program, ..] = pubkeys.as_slice() else {
                    return ::anyhow::__private::Err({
                        let error = ::anyhow::__private::format_err(
                            format_args!("Not enough accounts"),
                        );
                        error
                    });
                };
                if ::anyhow::__private::not(taker == &order.taker) {
                    return ::anyhow::__private::Err({
                        let error = ::anyhow::__private::format_err(
                            format_args!("Invalid taker"),
                        );
                        error
                    });
                }
                if ::anyhow::__private::not(maker == &order.maker) {
                    return ::anyhow::__private::Err({
                        let error = ::anyhow::__private::format_err(
                            format_args!("Invalid maker"),
                        );
                        error
                    });
                }
                if ::anyhow::__private::not(input_mint == &order.input_mint) {
                    return ::anyhow::__private::Err({
                        let error = ::anyhow::__private::format_err(
                            format_args!("Invalid input mint"),
                        );
                        error
                    });
                }
                if ::anyhow::__private::not(output_mint == &order.output_mint) {
                    return ::anyhow::__private::Err({
                        let error = ::anyhow::__private::format_err(
                            format_args!("Invalid output mint"),
                        );
                        error
                    });
                }
                let fill_ix = order_engine::client::args::Fill::try_from_slice(ix_data)
                    .map_err(|e| ::anyhow::__private::must_use({
                        let error = ::anyhow::__private::format_err(
                            format_args!("Invalid fill ix data {0}", e),
                        );
                        error
                    }))?;
                if fill_ix.input_amount != order.in_amount
                    || fill_ix.output_amount != order.out_amount
                {
                    return ::anyhow::__private::Err({
                        let error = ::anyhow::__private::format_err(
                            format_args!("Invalid fill ix"),
                        );
                        error
                    });
                }
                if ::anyhow::__private::not(fill_ix.expire_at == order.expire_at) {
                    return ::anyhow::__private::Err({
                        let error = ::anyhow::__private::format_err(
                            format_args!("Incorrect expiry"),
                        );
                        error
                    });
                }
            } else {
                return ::anyhow::__private::Err({
                    let error = ::anyhow::__private::format_err(
                        format_args!("Unexpected program id {0}", program_id),
                    );
                    error
                });
            }
        }
        if ::anyhow::__private::not(fill_ix_found) {
            return ::anyhow::__private::Err({
                let error = ::anyhow::__private::format_err(
                    format_args!("Missing fill instruction"),
                );
                error
            });
        }
        Ok(ValidatedFill {
            compute_unit_limit: compute_unit_limit
                .context("Missing compute unit limit")?,
            compute_unit_price: compute_unit_price.context("Missing compute unit price")?,
        })
    }
    pub struct ValidatedSimilarFill {
        pub taker: Pubkey,
        pub input_amount: u64,
        pub input_mint: Pubkey,
        pub taker_input_mint_token_account: Pubkey,
        pub expire_at: i64,
    }
    /// Given the original sanitized message, allow some minor changes
    pub fn validate_similar_fill_sanitized_message(
        sanitized_message: SanitizedMessage,
        original_sanitized_message: SanitizedMessage,
    ) -> Result<ValidatedSimilarFill> {
        let original_num_required_signatures = original_sanitized_message
            .header()
            .num_required_signatures;
        if ::anyhow::__private::not(
            sanitized_message.header().num_required_signatures
                == original_num_required_signatures,
        ) {
            return ::anyhow::__private::Err({
                let error = ::anyhow::__private::format_err(
                    format_args!("Num required signatures did not match"),
                );
                error
            });
        }
        for (original_signer, signer) in original_sanitized_message
            .account_keys()
            .iter()
            .zip(sanitized_message.account_keys().iter())
            .take(usize::from(original_num_required_signatures))
        {
            if ::anyhow::__private::not(signer == original_signer) {
                return ::anyhow::__private::Err({
                    let error = ::anyhow::__private::format_err(
                        format_args!("Signer did not match"),
                    );
                    error
                });
            }
        }
        let sanitized_instructions = sanitized_message.decompile_instructions();
        let original_instructions = original_sanitized_message.decompile_instructions();
        if ::anyhow::__private::not(
            sanitized_instructions.len() == original_instructions.len(),
        ) {
            return ::anyhow::__private::Err({
                let error = ::anyhow::__private::format_err(
                    format_args!("Number of instructions did not match"),
                );
                error
            });
        }
        let mut validated_similar_fill = None;
        let mut compute_unit_price = None;
        for (
            index,
            (
                BorrowedInstruction { program_id, accounts, data },
                BorrowedInstruction {
                    program_id: original_program_id,
                    accounts: original_accounts,
                    data: original_data,
                },
            ),
        ) in sanitized_instructions.into_iter().zip(original_instructions).enumerate() {
            if program_id == original_program_id
                && original_program_id == &compute_budget::ID
            {
                let compute_budget_ix = try_from_slice_unchecked::<
                    ComputeBudgetInstruction,
                >(data)?;
                match compute_budget_ix {
                    ComputeBudgetInstruction::SetComputeUnitLimit(_) => {}
                    ComputeBudgetInstruction::SetComputeUnitPrice(price) => {
                        if ::anyhow::__private::not(compute_unit_price.is_none()) {
                            return ::anyhow::__private::Err({
                                let error = ::anyhow::__private::format_err(
                                    format_args!("Comput unit price is already set"),
                                );
                                error
                            });
                        }
                        compute_unit_price = Some(price);
                        continue;
                    }
                    _ => {
                        return ::anyhow::__private::Err({
                            let error = ::anyhow::__private::format_err(
                                format_args!("Unexpected compute budget instruction"),
                            );
                            error
                        });
                    }
                }
            }
            if ::anyhow::__private::not(accounts.len() == original_accounts.len()) {
                return ::anyhow::__private::Err({
                    let error = ::anyhow::__private::format_err(
                        format_args!("Account len did not match"),
                    );
                    error
                });
            }
            if program_id != original_program_id
                || accounts
                    .iter()
                    .zip(original_accounts)
                    .any(|(accounts, original_accounts)| {
                        accounts.pubkey != original_accounts.pubkey
                            || accounts.is_signer != original_accounts.is_signer
                            || accounts.is_writable != original_accounts.is_writable
                    }) || data != original_data
            {
                return ::anyhow::__private::Err({
                    let error = ::anyhow::__private::format_err(
                        format_args!(
                            "Instruction did not match the original at index {0}, {1}",
                            index,
                            original_program_id,
                        ),
                    );
                    error
                });
            }
            if program_id == &order_engine::ID {
                if ::anyhow::__private::not(validated_similar_fill.is_none()) {
                    return ::anyhow::__private::Err({
                        let error = ::anyhow::__private::format_err(
                            format_args!("Duplicated fill instruction"),
                        );
                        error
                    });
                }
                if ::anyhow::__private::not(data.len() >= 8) {
                    return ::anyhow::__private::Err({
                        let error = ::anyhow::__private::format_err(
                            format_args!("Not enough data in fill instruction"),
                        );
                        error
                    });
                }
                let (discriminator, ix_data) = data.split_at(8);
                if ::anyhow::__private::not(
                    discriminator == order_engine::client::args::Fill::DISCRIMINATOR,
                ) {
                    return ::anyhow::__private::Err({
                        let error = ::anyhow::__private::format_err(
                            format_args!("Not a fill discriminator"),
                        );
                        error
                    });
                }
                let fill_ix = order_engine::client::args::Fill::try_from_slice(ix_data)
                    .map_err(|e| ::anyhow::__private::must_use({
                        let error = ::anyhow::__private::format_err(
                            format_args!("Invalid fill ix data {0}", e),
                        );
                        error
                    }))?;
                let taker = accounts.first().context("Invalid fill ix data")?.pubkey;
                let input_mint = accounts.get(6).context("Invalid fill ix data")?.pubkey;
                let taker_input_mint_token_account = accounts
                    .get(2)
                    .context("Invalid taker input mint token account ix data")?
                    .pubkey;
                validated_similar_fill = Some(ValidatedSimilarFill {
                    taker: *taker,
                    input_amount: fill_ix.input_amount,
                    input_mint: *input_mint,
                    taker_input_mint_token_account: *taker_input_mint_token_account,
                    expire_at: fill_ix.expire_at,
                });
            }
        }
        validated_similar_fill.context("Missing validated fill instruction")
    }
}
pub mod transaction {
    use anyhow::{anyhow, Result};
    use base64::prelude::*;
    use bincode;
    use solana_sdk::{
        message::{
            v0::LoadedAddresses, SanitizedMessage, SanitizedVersionedMessage,
            SimpleAddressLoader, VersionedMessage,
        },
        reserved_account_keys::ReservedAccountKeys, transaction::VersionedTransaction,
    };
    pub struct TransactionDetails {
        pub versioned_transaction: VersionedTransaction,
        pub sanitized_message: SanitizedMessage,
    }
    pub fn deserialize_transaction_base64_into_transaction_details(
        transaction: &str,
    ) -> Result<TransactionDetails> {
        let base64_decoded_tx = BASE64_STANDARD
            .decode(transaction)
            .map_err(|error| ::anyhow::__private::must_use({
                let error = ::anyhow::__private::format_err(
                    format_args!("Invalid transaction: {0}", error),
                );
                error
            }))?;
        let versioned_transaction = bincode::deserialize::<
            VersionedTransaction,
        >(&base64_decoded_tx)
            .map_err(|error| ::anyhow::__private::must_use({
                let error = ::anyhow::__private::format_err(
                    format_args!("Invalid transaction: {0}", error),
                );
                error
            }))?;
        let sanitized_message = versioned_message_to_sanitized_message(
            versioned_transaction.message.clone(),
        )?;
        Ok(TransactionDetails {
            versioned_transaction,
            sanitized_message,
        })
    }
    pub fn versioned_message_to_sanitized_message(
        versioned_message: VersionedMessage,
    ) -> Result<SanitizedMessage> {
        let sanitized_versioned_message = SanitizedVersionedMessage::try_new(
                versioned_message,
            )
            .map_err(|error| ::anyhow::__private::must_use({
                let error = ::anyhow::__private::format_err(
                    format_args!("Invalid transaction: {0}", error),
                );
                error
            }))?;
        let sanitized_message = SanitizedMessage::try_new(
                sanitized_versioned_message,
                SimpleAddressLoader::Enabled(LoadedAddresses::default()),
                &ReservedAccountKeys::empty_key_set(),
            )
            .map_err(|error| ::anyhow::__private::must_use({
                let error = ::anyhow::__private::format_err(
                    format_args!("Invalid transaction: {0}", error),
                );
                error
            }))?;
        Ok(sanitized_message)
    }
}
