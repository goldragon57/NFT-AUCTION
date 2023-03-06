use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Storage, StdResult, Decimal, Uint128, Order};
use cosmwasm_storage::{
    bucket, bucket_read, Bucket, ReadonlyBucket, ReadonlySingleton,
    Singleton, prefixed
};
use cw_storage_plus::Map;
use crate::asset::Asset;

pub static CONFIG_KEY: &[u8] = b"config";
pub static LIST_RESOLVER_KEY: &[u8] = b"listingresolver";
pub static CONFIG_MINTER: &[u8] = b"minters";
pub static CONFIG_NFT: &[u8] = b"nft";

// pub const OFFERINGS_COUNT: Item<u64> = Item::new(b"num_offerings");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub listing_count: u64,
    pub owner: String,
    pub max_aution_duration_blocks: u64,
}

pub fn store_config(storage: &mut dyn Storage, data: &Config) -> StdResult<()> {
    Singleton::new(storage, CONFIG_KEY).save(data)
}


pub fn read_config(storage: &dyn Storage) -> StdResult<Config> {
    ReadonlySingleton::new(storage, CONFIG_KEY).load()
}

pub fn store_minters(storage: &mut dyn Storage, minter: Addr, minter_info: MinterInfo) -> StdResult<()> {
    bucket(storage, CONFIG_MINTER).save(minter.as_bytes(), &minter_info)
}

pub fn remove_minter(storage: &mut dyn Storage, minter: Addr) -> StdResult<()> {
    prefixed(storage, CONFIG_MINTER).remove(minter.as_bytes());
    Ok(())
}

pub fn store_nft_address(storage: &mut dyn Storage, nft_address: &Addr) -> StdResult<()> {
    Singleton::new(storage, CONFIG_NFT).save(nft_address)
}

pub fn read_nft_address(storage: &dyn Storage) -> StdResult<Addr> {
    ReadonlySingleton::new(storage, CONFIG_NFT).load()
}

pub const MINTERS: Map<&str, MinterInfo> = Map::new("minters");

pub fn read_minters(storage: &dyn Storage) -> StdResult<Vec<String>> {
    MINTERS
        .keys(storage, None, None, Order::Ascending)
        .collect()
}

pub fn read_minter_info(storage: &dyn Storage, minter: Addr) -> Option<MinterInfo> {
    match bucket_read(storage, CONFIG_MINTER).load(minter.as_bytes()){
        Ok(v) => Some(v),
        _ => None
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Listing {
    pub token_id: String,

    pub contract_addr: Addr,

    pub seller: Addr,

    pub max_bid: Asset,

    pub max_bidder: Addr,

    pub block_limit: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Royalty {
  pub address: String,
  pub royalty_rate: Decimal
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MinterInfo {
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Metadata{
    // Identifies the asset to which this NFT represents
    pub name: String,
    // Describes the asset to which this NFT represents (may be empty)
    pub description: Option<String>,
    // An external URI
    pub external_link: Option<String>,
    // A collection this NFT belongs to
    pub collection: Option<Uint128>,
    // # of real piece representations
    pub num_real_repr: Uint128,
    // # of collectible nfts
    pub num_nfts: Uint128,
    // royalties
    pub royalties: Vec<Royalty>,
    // initial ask price
    pub init_price: Uint128
}

pub fn list_resolver(storage: &mut dyn Storage) -> Bucket<Listing> {
    bucket(storage, LIST_RESOLVER_KEY)
}

pub fn list_resolver_read(storage: &dyn Storage) -> ReadonlyBucket<Listing> {
    bucket_read(storage, LIST_RESOLVER_KEY)
}