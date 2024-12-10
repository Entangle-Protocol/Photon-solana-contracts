pub mod commons;
pub mod enums;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use commons::*;
pub use enums::*;
pub use instructions::*;
pub use state::*;

declare_id!("E81J4bjf8vHpioEGn75ysw895khGs5uaK3hDxPCe2z55");

#[constant]
pub const GENOME_ROOT: &[u8] = b"genome-root";

#[program]
pub mod genome {
    use super::*;
    use Role;

    pub fn receive_photon_msg<'c, 'info>(
        ctx: Context<'_, '_, 'c, 'info, PhotonMsg<'info>>,
        _op_hash: Vec<u8>,
        code: Vec<u8>,
        params: Vec<u8>,
    ) -> Result<()> {
        handle_photon_msg(ctx, code, params)
    }

    //
    //
    // INIT/CONFIG INSTRUCTIONS
    //
    //
    //
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        initialize::handler(ctx)
    }

    pub fn initialize_bridge(
        ctx: Context<InitializeBridge>,
        bridge_router_address: Vec<u8>,
    ) -> Result<()> {
        handle_initialize_bridge(ctx, bridge_router_address)
    }

    pub fn set_tournament_params(
        ctx: Context<SetTournamentParams>,
        minimal_admision_fee: u64,
        minimal_sponsor_pool: u64,
    ) -> Result<()> {
        handle_set_tournament_params(ctx, minimal_admision_fee, minimal_sponsor_pool)
    }

    pub fn set_games_minimal_fee(
        ctx: Context<SetGamesMinimalFee>,
        minimal_wager: u64,
    ) -> Result<()> {
        handle_set_games_minimal_fee(ctx, minimal_wager)
    }

    pub fn approve_operator(ctx: Context<ApproveOperator>, role: Role) -> Result<()> {
        handle_approve_operator(ctx, role)
    }

        //
    //
    // TOURNAMENT SINGLECHAIN INSTRUCTIONS
    //
    //
    //
    // Create Tournament
    pub fn create_tournament_singlechain(
        ctx: Context<CreateTournamentSinglechain>,
        organizer: Pubkey,
        params: TournamentParams,
    ) -> Result<()> {
        handle_create_tournament_singlechain(ctx, organizer, params)
    }

    // Register Tournament
    pub fn register_tournament_singlechain<'info>(
        ctx: Context<'_, '_, 'info, 'info, RegisterTournamentSinglechain<'info>>,
    ) -> Result<()> {
        handle_register_tournament_singlechain(ctx)
    }
    pub fn register_participant_to_tournament_singlechain<'info>(
        ctx: Context<'_, '_, 'info, 'info, RegisterParticipantToTournamentSinglechain<'info>>,
        teammate: Pubkey,
    ) -> Result<()> {
        handle_register_participant_to_tournament(ctx, teammate)
    }
    // this specific refund is for uncompleted teams before start tournament
    pub fn refund_participant_cancelation_singlechain(ctx: Context<RefundParticipantCancelationSinglechain>) -> Result<()> {
        handle_refund_participant_cancelation_singlechain(ctx)
    }

    // Start Tournament
    pub fn team_registration_verification(
        ctx: Context<TeamRegistrationVerification>,
    ) -> Result<()> {
        handle_team_registration_verification(ctx)
    }
    pub fn start_tournament(ctx: Context<StartTournament>) -> Result<()> {
        handle_start_tournament(ctx)
    }

    // Cancel Tournament
    pub fn refund_organizer_cancelation_singlechain(
        ctx: Context<RefundOrganizerCancelationSinglechain>,
    ) -> Result<()> {
        handle_refund_organizer_cancelation_singlechain(ctx)
    }
    pub fn cancel_tournament(ctx: Context<CancelTournament>) -> Result<()> {
        handle_cancel_tournament(ctx)
    }
    pub fn team_participant_registration_refund_singlechain(ctx: Context<TeamParticipantRegistrationRefundSinglechain>) -> Result<()> {
        handle_team_participant_registration_refund_singlechain(ctx)
    }

    // Finish Tournament
    pub fn finish_tournament(
        ctx: Context<FinishTournament>,
        rewards_prize_fractions: Vec<u16>,
        fee_type: u8,
        winners: Vec<Pubkey>,
    ) -> Result<()> {
        handle_finish_tournament(ctx, winners, rewards_prize_fractions, fee_type)
    }
    pub fn deliver_finish_organizer_tokens_singlechain(
        ctx: Context<DeliverFinishOrganizerTokensSinglechain>,
    ) -> Result<()> {
        handle_deliver_finish_organizer_tokens_singlechain(ctx)
    }
    pub fn deliver_participant_tokens_singlechain(ctx: Context<DeliverParticipantTokensSinglechain>) -> Result<()> {
        handle_deliver_participant_tokens_singlechain(ctx)
    }

    //
    //
    //  OMNICHAIN INSTRUCTIONS
    //
    //
    //
    // Register Tournament
    pub fn register_tournament_omnichain<'info>(
        ctx: Context<'_, '_, 'info, 'info, RegisterTournamentOmnichain<'info>>,
        teammates: Vec<Pubkey>,
    ) -> Result<()> {
        handle_register_tournament_omnichain(ctx, teammates)
    }

    // Create Tournament
    pub fn create_tournament_omnichain(
        ctx: Context<CreateTournamentOmnichain>,
        organizer: Pubkey,
        params: TournamentParams,
    ) -> Result<()> {
        handle_create_tournament_omnichain(ctx, organizer, params)
    }

    // this specific refund is for uncompleted teams before start tournament
    pub fn refund_participant_cancelation_omnichain(
        ctx: Context<RefundParticipantCancelationOmnichain>,
        participant: Pubkey,
    ) -> Result<()> {
        handle_refund_participant_cancelation_omnichain(ctx, participant)
    }

    // Cancel Tournament
    pub fn refund_organizer_cancelation_omnichain(
        ctx: Context<RefundOrganizerCancelationOmnichain>,
    ) -> Result<()> {
        handle_refund_organizer_cancelation_omnichain(ctx)
    }
    pub fn team_participant_registration_refund_omnichain(
        ctx: Context<TeamParticipantRegistrationRefundOmnichain>,
        participant: Pubkey,
    ) -> Result<()> {
        handle_team_participant_registration_refund_omnichain(ctx, participant)
    }

    // Finish Tournament
    pub fn deliver_finish_organizer_tokens_omnichain(
        ctx: Context<DeliverFinishOrganizerTokensOmnichain>,
    ) -> Result<()> {
        handle_deliver_finish_organizer_tokens_omnichain(ctx)
    }
    pub fn deliver_participant_tokens_omnichain(ctx: Context<DeliverParticipantTokensOmnichain>) -> Result<()> {
        handle_deliver_participant_tokens_omnichain(ctx)
    }


    //
    //
    //  FEE INSTRUCTIONS
    //
    //
    //
    pub fn set_fee_params(
        ctx: Context<SetFeeParams>,
        fee_type: u16,
        wallet: Pubkey,
        base_fee_param: u64,
        beneficiaries: Vec<Pubkey>,
        fractions: Vec<u64>,
        base_fee_meta: u64,
    ) -> Result<()> {
        set_fee_params::handle_set_fee_params(
            ctx,
            fee_type,
            wallet,
            base_fee_param,
            beneficiaries,
            fractions,
            base_fee_meta,
        )
    }

    pub fn claim_beneficiary_fees(ctx: Context<ClaimBeneficiaryFees>, fee_type: u16) -> Result<()> {
        handle_claim_beneficiary_fees(ctx, fee_type)
    }

    //
    //
    //  GAMES INSTRUCTIONS
    //
    //
    //
    pub fn create_game_omnichain(
        ctx: Context<CreateGameOmnichain>,
        game_id: u64,
        wager: u64,
        participants: Vec<Pubkey>,
        start_game: bool,
    ) -> Result<()> {
        handle_create_game_omnichain(ctx, game_id, wager, participants, start_game)
    }

    pub fn create_game_singlechain<'info>(
        ctx: Context<'_, '_, '_, 'info, CreateGameSinglechain<'info>>,
        wager: u64,
    ) -> Result<()> {
        handle_create_game_singlechain(ctx, wager)
    }

    pub fn register_game_participants_singlechain<'info>(
        ctx: Context<'_, '_, '_, 'info, RegisterGameParticipantsSinglechain<'info>>,
        start_game: bool,
    ) -> Result<()> {
        register_game_participants_singlechain::handle_register_game_participants_singlechain(
            ctx, start_game,
        )
    }

    pub fn register_game_participants_omnichain(
        ctx: Context<RegisterGameParticipantsOmnichain>,
        participants: Vec<Pubkey>,
        start_game: bool,
    ) -> Result<()> {
        register_game_participants_omnichain::handle_register_game_participants_omnichain(
            ctx,
            participants,
            start_game,
        )
    }

    pub fn start_game_singlechain(ctx: Context<StartGameSinglechain>) -> Result<()> {
        start_game_singlechain::handle_start_game_singlechain(ctx)
    }

    pub fn finish_game<'info>(
        ctx: Context<'_, '_, 'info, 'info, FinishGame<'info>>,
        fee_type: u16,
        winners: Vec<Pubkey>,
        prize_fractions: Vec<u64>,
    ) -> Result<()> {
        handle_finish_game(ctx, fee_type, winners, prize_fractions)
    }

    pub fn cancel_game<'info>(ctx: Context<'_, '_, 'info, 'info, CancelGame<'info>>) -> Result<()> {
        handle_cancel_game(ctx)
    }

    //
    //
    //  TREASURY INSTRUCTIONS
    //
    //
    //
    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        handle_deposit(ctx, amount)
    }

    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        handle_withdraw(ctx, amount)
    }

    pub fn withdraw_rewards(ctx: Context<WithdrawRewards>) -> Result<()> {
        handle_withdraw_rewards(ctx)
    }

    pub fn update_claimable_rewards(
        ctx: Context<UpdateClaimableRewards>,
        user: Pubkey,
        amount: u64,
    ) -> Result<()> {
        handle_update_claimable_rewards(ctx, user, amount)
    }

    //
    //
    //  BOOKMAKER INSTRUCTIONS
    //
    //
    //
    pub fn make_bet(
        ctx: Context<MakeBet>,
        gambler: Pubkey,
        captain: Pubkey,
        tournament_id: u64,
        amount: u64,
    ) -> Result<()> {
        handle_make_bet(ctx, gambler, captain, tournament_id, amount)
    }

    pub fn claim_overbook_tokens(
        ctx: Context<ClaimOverbookTokens>,
        gambler: Pubkey,
        captain: Pubkey,
        tournament_id: u64,
    ) -> Result<()> {
        handle_claim_overbook_tokens(ctx, gambler, captain, tournament_id)
    }

    pub fn claim_finish_tokens(
        ctx: Context<ClaimFinishTokens>,
        gambler: Pubkey,
        captain: Pubkey,
        tournament_id: u64,
        fee_type: u16,
    ) -> Result<()> {
        handle_claim_finish_tokens(ctx, gambler, captain, tournament_id, fee_type)
    }

    pub fn claim_cancel_tokens(
        ctx: Context<ClaimCancelTokens>,
        gambler: Pubkey,
        captain: Pubkey,
        tournament_id: u64,
    ) -> Result<()> {
        handle_claim_cancel_tokens(ctx, gambler, captain, tournament_id)
    }

    pub fn set_minimal_bet(ctx: Context<SetMinimalBet>, minimal_bet: u64) -> Result<()> {
        handle_set_minimal_bet(ctx, minimal_bet)
    }
}
