use cosmwasm_std::{
    entry_point, to_binary, from_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    Response, StdError, StdResult, WasmMsg, Uint128, Decimal, QueryRequest, WasmQuery, Addr
};
use cw20::Cw20ReceiveMsg;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, ResolveListingResponse, GFMintMsg, Cw20HookMsg};
use crate::state::{store_config, read_config, store_minters, remove_minter, read_minters, read_minter_info, list_resolver, list_resolver_read, Config, Listing, MinterInfo, Metadata, store_nft_address, read_nft_address};
use cw721::{
    Cw721ExecuteMsg::{Approve, TransferNft},
    Expiration,
    TokensResponse
};
use crate::asset::{ Asset, AssetInfo };

use cw721_base::msg::{ ExecuteMsg as Cw721ExecuteMsg, MintMsg, QueryMsg as Cw721QueryMsg };
pub const DEFAULT_EXPIRE_BLOCKS: u64 = 50_000;  // in seconds

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, StdError> {
    let config_state = Config { 
        listing_count: 0,
        owner: info.sender.to_string(),
        max_aution_duration_blocks: DEFAULT_EXPIRE_BLOCKS,
    };
    // Initiate listing_id with 0
    store_config(deps.storage, &config_state)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        // Route messages to appropriate handlers
        ExecuteMsg::PlaceListing {
            id,
            minimum_bid,
        } => execute_place_listing(deps, env, info.clone(), id, minimum_bid, info.sender),
        ExecuteMsg::BidListing { listing_id, bid_price} => execute_bid_listing(deps, env, info.clone(), listing_id, bid_price, info.sender.clone()),
        ExecuteMsg::WithdrawListing { listing_id } => {
            execute_withdraw_listing(deps, env, info, listing_id)
        },
        ExecuteMsg::Mint(mint_msg) => execute_mint(deps, env, info, mint_msg),
        ExecuteMsg::UpdateMinter{ minter } => update_minters(deps, env, info, &minter),
        ExecuteMsg::RemoveMinter{ minter } => unregister_minter(deps, env, info, &minter),
        ExecuteMsg::ReceiveToken(msg) => receive_token(deps, env, info, msg),
        ExecuteMsg::SetNftAddress{nft_address} => set_nft_address(deps, env, info, nft_address),
    }
}

fn set_nft_address(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    nft_address: String
) -> Result<Response, ContractError> {
    let config = read_config(deps.storage)?;
    let owner = deps.api.addr_validate(&config.owner)?;

    if info.sender != owner {
        return Err(ContractError::Unauthorized{});
    }

    store_nft_address(deps.storage, &deps.api.addr_validate(&nft_address)?)?;
    Ok(Response::default())
}

fn update_minters(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    minter: &String
) -> Result<Response, ContractError> {
    let config = read_config(deps.storage)?;
    let owner = deps.api.addr_validate(&config.owner)?;

    if info.sender != owner {
        return Err(ContractError::Unauthorized{});
    }

    let minter_info = MinterInfo {};

    store_minters(deps.storage, deps.api.addr_validate(minter)?, minter_info)?;
    Ok(Response::default())
}

fn unregister_minter(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    minter: &String
) -> Result<Response, ContractError> {
    let config = read_config(deps.storage)?;
    let owner = deps.api.addr_validate(&config.owner)?;

    if info.sender != owner{
        return Err(ContractError::Unauthorized{});
    }

    remove_minter(deps.storage, deps.api.addr_validate(minter)?)?;
    Ok(Response::default())
}

fn receive_token(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    wrapper: Cw20ReceiveMsg
) -> Result<Response, ContractError>{
    let msg = from_binary(&wrapper.msg)?;
    
    let asset = Asset {
        info: AssetInfo::Token {contract_addr: info.sender.to_string()},
        amount: wrapper.amount
    };

    let sender = deps.api.addr_validate(&wrapper.sender)?;
    match msg {
        Cw20HookMsg::BidListing{ listing_id,} 
            => execute_bid_listing(deps, env, info, listing_id, asset, sender),
        Cw20HookMsg::PlaceListing{ id }
            => execute_place_listing(deps, env, info, id, asset, sender),
    }
}

fn execute_mint(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: GFMintMsg,
) -> Result<Response, ContractError> {
    // check if the sender is a whitelisted minter
    let minter_info = read_minter_info(deps.storage, info.sender);

    if minter_info == None {
        return Err(ContractError::Unauthorized{});
    }

    let nft_contract_address = read_nft_address(deps.storage)?;

    // check if royalties are set properly. sum of them must not be greater than 100%
    let mut sum_total_rate = Decimal::zero();

    for royalty in msg.royalties.iter() {
        sum_total_rate = sum_total_rate + (*royalty).royalty_rate;
    }

    if sum_total_rate > Decimal::one() {
        return Err(ContractError::InvalidRoyaltyRate {})
    }

    let mut config = read_config(deps.storage)?;
    config.listing_count = config.listing_count + 1;

    store_config(deps.storage, &config)?;

    let token_id: String = ["GF".to_string(), config.listing_count.to_string()].join(".");

    Ok(Response::new()
        .add_message(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: nft_contract_address.to_string(),
            msg: to_binary(&Cw721ExecuteMsg::Mint(MintMsg {
                token_id,
                owner: msg.owner,
                token_uri: msg.image_uri,
                extension: Metadata {
                    name: msg.name,
                    description: msg.description,
                    external_link: msg.external_link,
                    collection: Some(Uint128::from(1 as u128)),
                    num_real_repr: msg.num_real_repr,
                    num_nfts:msg.num_nfts,
                    royalties: msg.royalties,
                    init_price: msg.init_price
                }
            }))?,
            funds: vec![]
        }))
    )
}

pub fn execute_bid_listing(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    listing_id: String,
    bid_price: Asset,
    sender: Addr,
) -> Result<Response, ContractError> {
    // check if the bid_price is correct in case of native tokens
    bid_price.assert_sent_native_token_balance(&info)?;

    // Fetch listing from listing_id
    let key = listing_id.as_bytes();
    let mut listing = list_resolver_read(deps.storage).load(key)?;
    
    if listing.block_limit < env.block.height {
        return Err(ContractError::AuctionEnded {});
    }

    // check if the token type is identical to the one in the listing
    if bid_price.info != listing.max_bid.info {
        return Err(ContractError::AssetInfoMismatch{});
    }

    // check if current bid exceeds the previous one
 
    if bid_price.amount < listing.max_bid.amount {
        return Err(ContractError::InsufficientFundsSend{});
    } 

    // refund former bid
    let last_bid = listing.max_bid;
    let last_bidder = listing.max_bidder;
    let msg = last_bid.into_msg(last_bidder.clone())?;

    // update bidder
    listing.max_bidder = sender.clone();
    listing.max_bid = bid_price.clone();
    list_resolver(deps.storage).save(key, &listing)?;

    if env.contract.address != last_bidder {
    // return money to last bidder
        Ok(Response::new()
            .add_attribute("Bidding", listing_id)
            .add_message(msg))
    } else {
        Ok(Response::new().add_attribute("Bidding", listing_id))
    }

}

pub fn execute_place_listing(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    id: String,
    minimum_bid: Asset,
    sender: Addr,
) -> Result<Response, ContractError> {
    let nft_contract_address = read_nft_address(deps.storage)?;

    // update listing id in store
    let config_state = read_config(deps.storage)?;
    let listing_count = config_state.listing_count + 1;

    // Each auction has a limit for 50000 blocks
    let listing = Listing {
        token_id: id.clone(),
        contract_addr: nft_contract_address.clone(),
        seller: sender,
        max_bid: minimum_bid,
        max_bidder: env.contract.address.clone(),
        block_limit: env.block.height + config_state.max_aution_duration_blocks,
    };

    let key = listing_count.to_string();
    // save listing to store
    list_resolver(deps.storage).save(key.as_bytes(), &listing)?;

    // lock nft to contract
    Ok(Response::new()
        .add_attribute("place_listing", id.to_string())
        .add_messages(vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: nft_contract_address.to_string(),
                funds: vec![],
                msg: to_binary(&Approve {
                    spender: env.contract.address.to_string(),
                    token_id: id.clone(),
                    expires: Some(Expiration::AtHeight(env.block.height + config_state.max_aution_duration_blocks)),
                })?,
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: nft_contract_address.to_string(),
                funds: vec![],
                msg: to_binary(&TransferNft {
                    recipient: String::from(env.contract.address.as_str()),
                    token_id: id,
                })?,
            }),
        ]))
}

pub fn execute_withdraw_listing(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    listing_id: String,
) -> Result<Response, ContractError> {

    let key = listing_id.as_bytes();
    let listing = list_resolver_read(deps.storage).load(key)?;

    // Check if the auction ended or not
    if listing.block_limit >= env.block.height {
        return Err(ContractError::AuctionNotEnded {});
    }

    let mut msgs = vec![];
    // remove listing from the store
    list_resolver(deps.storage).remove(key);

    // If noone has put a bid then then seller will be sent back with his NFT
    // Transfer the locked NFT to highest bidder and bid amount to the seller
    if env.contract.address != listing.max_bidder {
        // transfer NFT to buyer
        msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: listing.contract_addr.to_string(),
            funds: vec![],
            msg: to_binary(&TransferNft {
                recipient: listing.max_bidder.to_string(),
                token_id: listing_id.clone(),
            })?,
        }));

        // distribute royalties
        let mut remain_amount = listing.max_bid.amount;

        let token_info: Metadata = query_nft_info(deps.as_ref(), env, listing.token_id)?;

        for royalty in token_info.royalties.iter() {
            msgs.push((Asset {
                info: listing.max_bid.info.clone(),
                amount: listing.max_bid.amount * royalty.royalty_rate
            }).into_msg(deps.api.addr_validate(&royalty.address)?)?);

            remain_amount = remain_amount.checked_sub(listing.max_bid.amount * royalty.royalty_rate)?;
        }

        // transfer remain amount to seller
        msgs.push((Asset {
            info: listing.max_bid.info,
            amount: remain_amount
        }).into_msg(listing.seller.clone())?);

        Ok(Response::new()
            .add_attribute("listing_sold", listing_id.to_string())
            .add_messages(msgs))
    } else {
        Ok(Response::new()
            .add_attribute("listing_unsold", listing_id.to_string())
            .add_messages(vec![CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: listing.contract_addr.to_string(),
                funds: vec![],
                msg: to_binary(&TransferNft {
                    recipient: listing.seller.to_string(),
                    token_id: listing_id.clone(),
                })?,
            }), 
            ]))
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&read_config(deps.storage)?),
        QueryMsg::ResolveListing { id } => query_list_resolver(deps, env, id),
        QueryMsg::QueryMinter {} => to_binary(&query_minters(deps, env)?),
        QueryMsg::QueryNftInfo {token_id} => to_binary(&query_nft_info(deps, env, token_id)?),
        QueryMsg::AllTokens{} => to_binary(&query_all_nft_ids(deps, env)?)
    }
}

pub fn query_nft_info(
    deps: Deps, 
    _env: Env,  
    token_id: String,
) -> StdResult<Metadata> {
    let nft_contract_address = read_nft_address(deps.storage)?;

    let nft_info: MintMsg<Metadata> = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: nft_contract_address.to_string(),
        msg: to_binary(&Cw721QueryMsg::NftInfo {token_id})?,
    }))?;

    let meta_data: Metadata = nft_info.extension;

    Ok(meta_data)
}

pub fn query_all_nft_ids(
    deps: Deps, 
    _env: Env,  
) -> StdResult<Vec<String>> {
    let nft_contract_address = read_nft_address(deps.storage)?;

    let token_ids: TokensResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: nft_contract_address.to_string(),
        msg: to_binary(&Cw721QueryMsg::AllTokens {start_after: None, limit: None})?,
    }))?;

    Ok(token_ids.tokens)
}

pub fn query_minters(deps: Deps, _env: Env) -> StdResult<Vec<String>> {
    read_minters(deps.storage)  
}

fn query_list_resolver(deps: Deps, _env: Env, id: String) -> StdResult<Binary> {
    // Fetch listing from listing_id
    let key = id.as_bytes();

    let resp = match list_resolver_read(deps.storage).may_load(key)? {
        Some(listing) => Some(listing),
        None => None,
    };
    let unwrapped_resp = resp.unwrap();
    let resolve_listing = ResolveListingResponse {
        token_id: unwrapped_resp.token_id,
        contract_addr: unwrapped_resp.contract_addr,
        seller: unwrapped_resp.seller,
        max_bid: unwrapped_resp.max_bid,
        max_bidder: unwrapped_resp.max_bidder,
        block_limit: unwrapped_resp.block_limit,
    };
    to_binary(&resolve_listing)
}
