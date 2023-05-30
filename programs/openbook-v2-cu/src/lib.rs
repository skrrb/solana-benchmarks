use anchor_lang::prelude::*;
use openbook_v2::state::{OutEvent, Side};
use solana_program::log::sol_log_compute_units;

mod state;
use state::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

fn random_positions() -> Vec<usize> {
    vec![
        1, 41, 223, 4, 2, 293, 300, 483, 10, 23, 45, 20, 146, 342, 123, 435, 112, 234, 211, 89,
    ]
}

#[program]
pub mod openbook_v2_cu {
    use super::*;

    pub fn ring_buf(ctx: Context<RingBuf>) -> Result<()> {
        let mut event_queue = ctx.accounts.event_queue.load_init()?;
        let random = random_positions();

        msg!("# Inserting_{}", MAX_NUM_EVENTS);
        sol_log_compute_units();
        for i in 0..MAX_NUM_EVENTS {
            let event = OutEvent::new(
                Side::Bid,
                0,
                0,
                event_queue.header.seq_num,
                Pubkey::from([i as u8; 32]),
                i.try_into().unwrap(),
            );
            event_queue.push_back(bytemuck::cast(event)).unwrap();
        }
        sol_log_compute_units();

        let target = Pubkey::from([1u8; 32]);
        let current_len = event_queue.header.count();

        msg!("# Removing_{}_random_positions", random.len());
        sol_log_compute_units();
        let mut sorted = random.clone();
        sorted.sort();
        for (i, pos) in sorted.into_iter().enumerate() {
            let position_after_resizes = pos - i;
            event_queue.buf.swap(0, position_after_resizes);
            event_queue.pop_front().unwrap();
        }
        sol_log_compute_units();

        let current_len = event_queue.header.count();
        for i in current_len..MAX_NUM_EVENTS {
            let event = OutEvent::new(
                Side::Bid,
                0,
                0,
                event_queue.header.seq_num,
                Pubkey::from([i as u8; 32]),
                i.try_into().unwrap(),
            );
            event_queue.push_back(bytemuck::cast(event)).unwrap();
        }

        msg!("# Iterating");
        sol_log_compute_units();
        assert_eq!(event_queue.header.count(), MAX_NUM_EVENTS);
        sol_log_compute_units();

        msg!("# Deleting_{}", event_queue.header.count());
        sol_log_compute_units();
        for _ in 0..event_queue.header.count() {
            event_queue.pop_front().unwrap();
        }
        sol_log_compute_units();

        Ok(())
    }

    pub fn d_l_list(ctx: Context<DLList>) -> Result<()> {
        let mut event_queue = ctx.accounts.event_queue.load_init()?;
        let random = random_positions();

        msg!("# Initialize");
        sol_log_compute_units();
        event_queue.init();
        sol_log_compute_units();

        msg!("# Inserting_{}", MAX_NUM_EVENTS);
        sol_log_compute_units();
        for i in 0..MAX_NUM_EVENTS {
            let event = OutEvent::new(
                Side::Bid,
                0,
                0,
                event_queue.header.seq_num,
                Pubkey::from([i as u8; 32]),
                i.try_into().unwrap(),
            );
            event_queue.push_back(bytemuck::cast(event));
        }
        sol_log_compute_units();

        msg!("# Removing_{}_random_positions", random.len());
        sol_log_compute_units();
        for pos in random {
            event_queue.delete_slot(pos).unwrap();
        }
        sol_log_compute_units();

        let current_len = event_queue.header.count();
        for i in current_len..MAX_NUM_EVENTS {
            let event = OutEvent::new(
                Side::Bid,
                0,
                0,
                event_queue.header.seq_num,
                Pubkey::from([i as u8; 32]),
                i.try_into().unwrap(),
            );
            event_queue.push_back(bytemuck::cast(event));
        }

        msg!("# Iterating");
        sol_log_compute_units();
        assert_eq!(event_queue.header.count(), MAX_NUM_EVENTS);
        sol_log_compute_units();

        msg!("# Deleting_{}", event_queue.header.count());
        sol_log_compute_units();
        for _ in 0..event_queue.header.count() {
            event_queue.delete().unwrap();
        }
        sol_log_compute_units();
        Ok(())
    }
}

#[derive(Accounts)]
pub struct RingBuf<'info> {
    #[account(zero)]
    event_queue: AccountLoader<'info, EventQueue>,
}

#[derive(Accounts)]
pub struct DLList<'info> {
    #[account(zero)]
    event_queue: AccountLoader<'info, DLLEventQueue>,
}

#[cfg(test)]
mod comp_budget {
    use super::*;
    use anchor_lang::InstructionData;
    use solana_program_test::{tokio, ProgramTest};
    use solana_sdk::{
        account::Account,
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        rent::Rent,
        signature::Signer,
        transaction::Transaction,
    };
    use std::mem::size_of;

    fn zero_account(len: usize) -> Account {
        Account {
            owner: crate::id(),
            lamports: Rent::default().minimum_balance(len),
            data: vec![0; len],
            ..Account::default()
        }
    }

    async fn send_instruction(
        context: &mut solana_program_test::ProgramTestContext,
        data: Vec<u8>,
        pubkey: Pubkey,
    ) {
        let accounts = vec![AccountMeta::new(pubkey, false)];
        let ix = Instruction::new_with_bytes(crate::id(), &data, accounts);
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&context.payer.pubkey()),
            &[&context.payer],
            context.last_blockhash,
        );

        context
            .banks_client
            .process_transactions(vec![tx])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn event_queue() {
        let ringbuf_pubkey = Pubkey::new_unique();
        let ringbuf_account = zero_account(8 + size_of::<crate::state::EventQueue>());

        let list_pubkey = Pubkey::new_unique();
        let list_account = zero_account(8 + size_of::<crate::state::DLLEventQueue>());

        let mut program = ProgramTest::default();
        program.add_program("openbook_v2_cu", crate::id(), None);
        program.add_account(ringbuf_pubkey, ringbuf_account);
        program.add_account(list_pubkey, list_account);

        let mut context = program.start_with_context().await;

        send_instruction(
            &mut context,
            crate::instruction::RingBuf {}.data(),
            ringbuf_pubkey,
        )
        .await;

        send_instruction(
            &mut context,
            crate::instruction::DLList {}.data(),
            list_pubkey,
        )
        .await;
    }
}
