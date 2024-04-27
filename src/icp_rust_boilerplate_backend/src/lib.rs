#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Ticket {
    id: u64,
    event: String,
    price: u64,
    seat: String,
    created_at: u64,
    updated_at: Option<u64>,
}

impl Storable for Ticket {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Ticket {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
           .expect("Cannot create a counter")
    );

    static STORAGE: RefCell<StableBTreeMap<u64, Ticket, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
        ));
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct TicketPayload {
    event: String,
    price: u64,
    seat: String,
}

#[ic_cdk::query]
fn get_ticket(id: u64) -> Result<Ticket, Error> {
    match _get_ticket(&id) {
        Some(ticket) => Ok(ticket),
        None => Err(Error::NotFound {
            msg: format!("a ticket with id={} not found", id),
        }),
    }
}

#[ic_cdk::update]
fn add_ticket(ticket: TicketPayload) -> Option<Ticket> {
    let id = ID_COUNTER
       .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
       .expect("cannot increment id counter");
    let ticket = Ticket {
        id,
        event: ticket.event,
        price: ticket.price,
        seat: ticket.seat,
        created_at: time(),
        updated_at: None,
    };
    do_insert(&ticket);
    Some(ticket)
}

#[ic_cdk::update]
fn update_ticket(id: u64, payload: TicketPayload) -> Result<Ticket, Error> {
    match STORAGE.with(|service| service.borrow().get(&id)) {
        Some(mut ticket) => {
            ticket.event = payload.event;
            ticket.price = payload.price;
            ticket.seat = payload.seat;
            ticket.updated_at = Some(time());
            do_insert(&ticket);
            Ok(ticket)
        }
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't update a ticket with id={}. ticket not found",
                id
            ),
        }),
    }
}

#[ic_cdk::update]
fn delete_ticket(id: u64) -> Result<Ticket, Error> {
    match STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(ticket) => Ok(ticket),
        None => Err(Error::NotFound {
            msg: format!(
                "couldn't delete a ticket with id={}. ticket not found.",
                id
            ),
        }),
    }
}

fn do_insert(ticket: &Ticket) {
    STORAGE.with(|service| service.borrow_mut().insert(ticket.id, ticket.clone()));
}

fn _get_ticket(id: &u64) -> Option<Ticket> {
    STORAGE.with(|service| service.borrow().get(id))
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
}

ic_cdk::export_candid!();