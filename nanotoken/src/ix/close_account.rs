use crate::solana_nostd_entrypoint::NoStdAccountInfo;
use bytemuck::{Pod, Zeroable};
use solana_program::{log, program_error::ProgramError};

use crate::{
    error::NanoTokenError, utils::split_at_unchecked, Mint, TokenAccount,
};

#[repr(C)]
pub struct CloseAccountArgs {}

impl BurnArgs {
    pub fn from_data<'a>(
        data: &mut &'a [u8],
    ) -> Result<&'a BurnArgs, ProgramError> {
        const IX_LEN: usize = core::mem::size_of::<BurnArgs>();
        if data.len() >= IX_LEN {
            // SAFETY:
            // We do the length check ourselves instead of via
            // core::slice::split_at so we can return an error
            // instead of panicking.
            let (ix_data, rem) = unsafe { split_at_unchecked(data, IX_LEN) };
            *data = rem;

            // This is always aligned and all bit patterns are valid
            Ok(unsafe { &*(ix_data.as_ptr() as *const BurnArgs) })
        } else {
            Err(ProgramError::InvalidInstructionData)
        }
    }
    pub fn size() -> usize {
        core::mem::size_of::<Self>()
    }
}

pub fn close_account(
    accounts: &[NoStdAccountInfo],
    args: &CloseAccountArgs,
) -> Result<usize, ProgramError> {
    log::sol_log("close_account");

    // Unpack accounts
    //
    // 1) target account needs an owner/disc check and an authority check before trying to close
    // 2) dest needs to have the same mint as target, and requires an owner/disc check
    // 3) owner must be target authority and must be signer
    let [target, dest, owner, _rem @ ..] = accounts else {
        log::sol_log("closing account expecting [target, dest owner, .. ]");
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Check that owner signed this
    if !owner.is_signer() {
        log::sol_log("target account owner must sign to close this account");
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Load target_account
    let (target_owner, target_mint, target_balance) =
        unsafe { TokenAccount::check_disc(target)? };
    let (_dest_owner, dest_mint, dest_balance) =
        unsafe { TokenAccount::check_disc(dest)? };

    // Check that the owner is correct
    if solana_program::program_memory::sol_memcmp(
        target_owner.as_ref(),
        owner.key().as_ref(),
        32,
    ) != 0
    {
        log::sol_log("incorrect target_account owner");
        return Err(ProgramError::IllegalOwner);
    }

    // Check that the mints match
    if target_mint != dest_mint {
        log::sol_log("target/dest mint mismatch");
        return Err(NanoTokenError::IncorrectMint.into());
    }

    // Transfering the token balance from target to dest
    if unsafe { *target_balance } > 0 {
        unsafe {
            *dest_balance += *target_balance;
            *target_balance = 0;
        }
    }

    // Transfer lamports to owner
    let target_ref = target.try_borrow_mut_lamports().expect("failed to borrow lamports from target account");
    let dest_ref = dest.try_borrow_mut_lamports().expect("failed to borrow lamports from dest account");
    
    dest_ref += target_ref;
    target_ref = 0;
    
    Ok(3)
}
