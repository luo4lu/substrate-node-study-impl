//! A collection of node-specific RPC methods.
//! Substrate provides the `sc-rpc` crate, which defines the core RPC layer
//! used by Substrate nodes. This file extends those RPC definitions with
//! capabilities that are specific to this project's runtime configuration.

#![warn(missing_docs)]

use std::sync::Arc;

use std::collections::BTreeMap;
use node_template_runtime::{opaque::Block, AccountId, Balance, BlockNumber, Hash, Index, TransactionConverter};
use pallet_contracts_rpc::{Contracts, ContractsApi};
pub use sc_rpc_api::DenyUnsafe;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_transaction_pool::TransactionPool;
use sc_client_api::{
    backend::{StorageProvider, Backend, StateBackend, AuxStore}
};
use fc_rpc_core::types::{PendingTransactions};
use sp_runtime::traits::BlakeTwo256;
use sc_network::NetworkService;
use pallet_ethereum::EthereumStorageSchema;
/// Full client dependencies.
pub struct FullDeps<C, P> {
    /// The client instance to use.
    pub client: Arc<C>,
    /// Transaction pool instance.
    pub pool: Arc<P>,
    /// Whether to deny unsafe calls
    pub deny_unsafe: DenyUnsafe,
    /// The Node authority flag
    pub is_authority: bool,
    /// Whether to enable dev signer
	pub enable_dev_signer: bool,
    /// Network service
	pub network: Arc<NetworkService<Block, Hash>>,
    /// Ethereum pending transactions.
	pub pending_transactions: PendingTransactions,
    /// Backend.
	pub backend: Arc<fc_db::Backend<Block>>,
    /// Maximum number of logs in a query.
	pub max_past_logs: u32,
}

/// Instantiate all full RPC extensions.
pub fn create_full<C, P, BE>(deps: FullDeps<C, P>) -> jsonrpc_core::IoHandler<sc_rpc::Metadata>
where
    BE: Backend<Block> + 'static,
    BE::State: StateBackend<BlakeTwo256>,
    C: ProvideRuntimeApi<Block> + StorageProvider<Block, BE> + AuxStore,
    C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError> + 'static,
    C: Send + Sync + 'static,
    C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Index>,
    C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
    C::Api: BlockBuilder<Block>,
    C::Api: pallet_contracts_rpc::ContractsRuntimeApi<Block, AccountId, Balance, BlockNumber, Hash>,
    C::Api: fp_rpc::EthereumRuntimeRPCApi<Block>,
    P: TransactionPool<Block=Block> + 'static,
{
    use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApi};
    use substrate_frame_rpc_system::{FullSystem, SystemApi};
    use fc_rpc::{EthApi, EthApiServer, EthSigner, EthDevSigner, OverrideHandle,
        SchemaV1Override, StorageOverride, RuntimeApiStorageOverride};

    let mut io = jsonrpc_core::IoHandler::default();
    let FullDeps {
        client,
        pool,
        deny_unsafe,
        is_authority,
        enable_dev_signer,
        network,
        pending_transactions,
        backend,
        max_past_logs,
    } = deps;

    io.extend_with(SystemApi::to_delegate(FullSystem::new(
        client.clone(),
        pool.clone(),
        deny_unsafe,
    )));

    io.extend_with(TransactionPaymentApi::to_delegate(TransactionPayment::new(
        client.clone(),
    )));

    // Extend this RPC with a custom API by using the following syntax.
    // `YourRpcStruct` should have a reference to a client, which is needed
    // to call into the runtime.
    // `io.extend_with(YourRpcTrait::to_delegate(YourRpcStruct::new(ReferenceToClient, ...)));`
    io.extend_with(ContractsApi::to_delegate(Contracts::new(client.clone())));

    let mut signers = Vec::new();
    if enable_dev_signer {
        signers.push(Box::new(EthDevSigner::new()) as Box<dyn EthSigner>);
    }
    let mut overrides_map = BTreeMap::new();
    overrides_map.insert(
        EthereumStorageSchema::V1, 
        Box::new(SchemaV1Override::new(client.clone())) as Box<dyn StorageOverride<_> + Send + Sync>,    
    );

    let overrides = Arc::new(OverrideHandle {
        schemas: overrides_map,
        fallback: Box::new(RuntimeApiStorageOverride::new(client.clone())),
    });
    io.extend_with(EthApiServer::to_delegate(EthApi::new(
        client.clone(),
        pool.clone(),
        TransactionConverter,
        network.clone(),
        pending_transactions.clone(),
        signers,
        overrides.clone(),
        backend.clone(),
        is_authority,
        max_past_logs,
    )));
    io
}
