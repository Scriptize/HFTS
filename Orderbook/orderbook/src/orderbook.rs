#![allow(unused)]

use std::{
    rc::Rc,
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    thread::{self, JoinHandle},
    sync::{Arc, Mutex, Condvar},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant, SystemTime}
};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum OrderType {
    GoodTillCancel,
    GoodForDay,
    FillAndKill,
    FillOrKill,
    Market,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Side {
    Buy,
    Sell,
}

type Price = i32;
type Quantity = u32;
type OrderId = u32;
#[derive(Debug)]
pub struct LevelInfo {
    pub price: Price,
    pub quantity: Quantity,
}

type LevelInfos = Vec<LevelInfo>;
#[derive(Debug)]
pub struct OrderbookLevelInfos {
    bid_infos: LevelInfos,
    ask_infos: LevelInfos,
}

impl OrderbookLevelInfos {
    pub fn new(bids: LevelInfos, asks: LevelInfos) -> Self {
        Self { bid_infos: bids, ask_infos: asks }
    }
    pub const fn get_bids(&self) -> &LevelInfos {
        &self.bid_infos
    }
    pub const fn get_asks(&self) -> &LevelInfos {
        &self.ask_infos
    }
}
#[derive(Debug)]
pub struct Order {
    order_type: OrderType,
    order_id: OrderId,
    side: Side,
    price: Price,
    initial_quantity: Quantity,
    remaining_quantity: Quantity,
    filled_quantity: Quantity,
    filled: bool,
}

impl Order {
    //new pointer to order; will be used most of the time
    pub fn new(
        order_type: OrderType,
        order_id: OrderId,
        side: Side,
        price: Price,
        quantity: Quantity,
    ) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self{
            order_type,
            order_id,
            side,
            price,
            initial_quantity: quantity,
            remaining_quantity: quantity,
            filled_quantity: 0,
            filled: false,
        }))
    }

    pub fn new_market(
        order_id: OrderId,
        side: Side,
        quantity: Quantity, 
    ) -> Rc<RefCell<Self>> {
        // Use an obviously invalid price for market orders, e.g., i32::MIN
        Self::new(
            OrderType::Market,
            order_id,
            side,
            i32::MIN,
            quantity
        )
    }

    pub fn to_good_till_cancel(&mut self, price: Price) -> Result<(), String> {
        if self.get_order_type() != OrderType::Market {
            return Err("Order cannot be filled for more than its remaining quantity.".to_string());
        }
        if self.get_price() == i32::MIN {
            return Err("Order must be a tradable price".to_string());
        }
        self.price = price;
        self.order_type = OrderType::GoodTillCancel;
        Ok(())
    }

    pub const fn get_order_id(&self) -> OrderId {
        self.order_id
    }
    pub const fn get_side(&self) -> Side {
        self.side
    }
    pub const fn get_price(&self) -> Price {
        self.price
    }
    pub const fn get_order_type(&self) -> OrderType {
        self.order_type
    }
    pub const fn get_initial_quantity(&self) -> Quantity {
        self.initial_quantity
    }
    pub const fn get_remaining_quantity(&self) -> Quantity {
        self.remaining_quantity
    }
    pub const fn get_filled_quantity(&self) -> Quantity {
        self.filled_quantity
    }
    pub const fn is_filled(&self) -> bool {
        self.filled
    }

    pub fn fill(&mut self, quantity: Quantity) -> Result<(), String> {
        if quantity <= self.remaining_quantity {
            self.remaining_quantity -= quantity;
            self.filled_quantity += quantity;
            if self.remaining_quantity == 0 {
                self.filled = true;
            }
            Ok(())
        } else {
            Err("Order cannot be filled for more than it's remaining quantity.".to_string())
        }
    }

    
}

type OrderPointer = Rc<RefCell<Order>>;
type OrderPointers = Vec<OrderPointer>;
#[derive(Debug)]
pub struct OrderModify {
    order_id: OrderId,
    price: Price,
    side: Side,
    quantity: Quantity,
}

impl OrderModify {
    pub fn new(order_id: OrderId, side: Side, price: Price, quantity: Quantity) -> Self {
        Self {
            order_id,
            side,
            price,
            quantity,
        }
    }

    pub const fn get_order_id(&self) -> OrderId {
        self.order_id
    }
    pub const fn get_side(&self) -> Side {
        self.side
    }
    pub const fn get_price(&self) -> Price {
        self.price
    }
    pub const fn get_quantity(&self) -> Quantity {
        self.quantity
    }

    pub fn to_order_pointer(&self, order_type: OrderType) -> OrderPointer {
        Order::new(
            order_type,
            self.get_order_id(),
            self.get_side(),
            self.get_price(),
            self.get_quantity(),
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TradeInfo {
    pub order_id: OrderId,
    pub price: Price,
    pub quantity: Quantity,
}
#[derive(Debug)]
pub struct Trade{
    bid_trade: TradeInfo,
    ask_trade: TradeInfo,
}

impl Trade{
    pub fn new(bid_trade: TradeInfo, ask_trade: TradeInfo) -> Self{
        Self{
            bid_trade,
            ask_trade,
        }
    }

    pub const fn get_bid_trade(&self) -> TradeInfo {
        self.bid_trade
    }

    pub const fn get_ask_trade(&self) -> TradeInfo {
        self.ask_trade
    }
}

type Trades = Vec<Trade>;

///////////////////////////////////////
#[derive(Debug)]
struct OrderEntry{
    order: OrderPointer,
    location: usize,
}
#[derive(Debug)]
pub struct Orderbook{
    bids: BTreeMap<Price, OrderPointers>,
    asks: BTreeMap<Price, OrderPointers>,
    orders: HashMap<OrderId, OrderEntry>,
    orders_prune_thread: Option<JoinHandle<()>>,
    shutdown_condition_variable: Condvar,
    shutdown: AtomicBool,

}

impl Orderbook{
    pub fn new(
        bids: BTreeMap<Price, OrderPointers>,
        asks: BTreeMap<Price, OrderPointers>,
    ) -> Self {
        Self {
            bids,
            asks,
            orders: HashMap::new(),
            orders_prune_thread: None,
            shutdown_condition_variable: Condvar::new(),
            shutdown: AtomicBool::new(false),
        }
    }

    /// Creates an `Arc<Mutex<Orderbook>>` and starts the prune thread.
    pub fn build(
        bids: BTreeMap<Price, OrderPointers>,
        asks: BTreeMap<Price, OrderPointers>,
    ) -> Arc<Mutex<Self>> {
        let orderbook = Arc::new(Mutex::new(Orderbook::new(bids, asks)));
        let orderbook_clone = Arc::clone(&orderbook);

        // Start the prune thread after construction
        let handle = thread::spawn(move || {
            let mut ob = orderbook_clone.lock().unwrap();
            ob.prune_gfd_orders();
        });

        // Store the handle in the struct
        {
            let mut ob = orderbook.lock().unwrap();
            ob.orders_prune_thread = Some(handle);
        }

        orderbook
    }


    pub fn size(&self) -> usize {
        self.orders.len()
    }

    pub fn get_order_infos(&self) -> OrderbookLevelInfos{
        let mut bid_infos: LevelInfos = Vec::with_capacity(self.orders.len());
        let mut ask_infos: LevelInfos = Vec::with_capacity(self.orders.len());

        let create_level_infos = |price: Price , orders: OrderPointers|{
            let total_quantity = orders.iter().fold(0, |running_sum, order|{
                running_sum + order.borrow().get_remaining_quantity()
            });

            LevelInfo{
                price,
                quantity: total_quantity,
            }
        };

        for (price, orders) in &self.bids{
            bid_infos.push(create_level_infos(*price, orders.clone()));
        }

        for (price, orders) in &self.asks{
            ask_infos.push(create_level_infos(*price, orders.clone()));
        }

        OrderbookLevelInfos{
            bid_infos,
            ask_infos,
        }
    }

    pub fn add_order(&mut self, order: OrderPointer) -> Trades{
        //check if order exist
        if self.orders.contains_key(&order.borrow().get_order_id()){
            return vec![];
        }

        if order.borrow().get_order_type() == OrderType::Market{
            if order.borrow().get_side() == Side::Buy && !self.asks.is_empty(){
                let (worst_ask, _) = self.asks.iter().next().unwrap();
            }
            else if order.borrow().get_side() == Side::Buy && !self.asks.is_empty(){
                let (worst_ask, _) = self.asks.iter().next().unwrap();
            }
            else{
                return vec![]
            }
        }
        //check if order is a FillAndKill that can't match
        if order.borrow().get_order_type() == OrderType::FillAndKill && !self.can_match(order.borrow().get_side(), order.borrow().get_price()){
            return vec![];
        }

        let mut index: usize = 0;

        if order.borrow().get_side() == Side::Buy{
            let orders = &mut self.bids.entry(order.borrow().get_price()).or_default();
            orders.push(order.clone());
            index = orders.len() - 1;
        } else {
            let orders = &mut self.asks.entry(order.borrow().get_price()).or_default();
            orders.push(order.clone());
            index = orders.len() - 1;
        }

        let order_id = order.borrow().get_order_id();

        self.orders.insert(order_id, OrderEntry { order, location: index });
        self.match_orders()
    }

    pub fn cancel_order(&mut self, order_id: OrderId) {
        if let Some(entry) = self.orders.remove(&order_id) {
            let price = entry.order.borrow().get_price();
            let side = entry.order.borrow().get_side();
            let location = entry.location;
    
            let maybe_queue = match side {
                Side::Buy => self.bids.get_mut(&price),
                Side::Sell => self.asks.get_mut(&price),
            };
    
            if let Some(queue) = maybe_queue {
                let last_index = queue.len() - 1;
                queue.swap_remove(location);
    
                // Fix the index of the order that was moved (if not the one we removed)
                if location < queue.len() {
                    let moved_order = &queue[location];
                    let moved_id = moved_order.borrow().get_order_id();
                    if let Some(moved_entry) = self.orders.get_mut(&moved_id) {
                        moved_entry.location = location;
                    }
                }
                
                // If queue is empty now, remove the price level
                if queue.is_empty() {
                    match side {
                        Side::Buy => { self.bids.remove(&price); }
                        Side::Sell => { self.asks.remove(&price); }
                    }
                }
            }
        }
    }

    pub fn modify_order(&mut self, order: OrderModify) -> Trades{
        if !self.orders.contains_key(&order.get_order_id()){
            return vec![];
        }

        let order_type = if let Some(entry) = self.orders.get(&order.get_order_id()) {
            entry.order.borrow().get_order_type()
        } else {
            return vec![];
        };
        self.cancel_order(order.get_order_id());

        self.add_order(order.to_order_pointer(order_type))
    }

    fn can_match(&self, side: Side, price: Price) -> bool{
        match side{
            Side::Buy => {
                if self.asks.is_empty(){
                    return false;
                }
                let (best_ask, _) = self.asks.iter().next().unwrap();
                return price >= *best_ask;
            }

            Side::Sell => {
                if self.bids.is_empty(){
                    return false;
                }
                let (best_bid, _) = self.bids.iter().next().unwrap();
                return price <= *best_bid;
            }
        }
    }

    fn match_orders(&mut self) -> Trades {
        let mut trades: Trades = Vec::with_capacity(self.orders.len());

        loop {
            if self.bids.is_empty() || self.asks.is_empty() {
                break;
            }

            // Get best bid and ask (highest bid, lowest ask)
            let (bid_price, bids) = match self.bids.iter_mut().next_back() {
                Some((p, b)) => (*p, b),
                None => break,
            };
            let (ask_price, asks) = match self.asks.iter_mut().next() {
                Some((p, a)) => (*p, a),
                None => break,
            };

            if bid_price < ask_price {
                break;
            }

            // Always match the first order at each price level
            let bid_order_ptr = &bids[0];
            let ask_order_ptr = &asks[0];

            let mut bid = bid_order_ptr.borrow_mut();
            let mut ask = ask_order_ptr.borrow_mut();

            let trade_quantity = bid.get_remaining_quantity().min(ask.get_remaining_quantity());

            // Fill both orders
            bid.fill(trade_quantity).ok();
            ask.fill(trade_quantity).ok();

            // Prepare trade info
            trades.push(Trade::new(
                TradeInfo {
                    order_id: bid.get_order_id(),
                    price: bid.get_price(),
                    quantity: trade_quantity,
                },
                TradeInfo {
                    order_id: ask.get_order_id(),
                    price: ask.get_price(),
                    quantity: trade_quantity,
                },
            ));

            // Remove filled orders from book and orders map
            let mut remove_bid = false;
            let mut remove_ask = false;
            if bid.is_filled() {
                let bid_id = bid.get_order_id();
                remove_bid = true;
                self.orders.remove(&bid_id);
            }
            if ask.is_filled() {
                let ask_id = ask.get_order_id();
                remove_ask = true;
                self.orders.remove(&ask_id);
            }
            drop(bid);
            drop(ask);

            if remove_bid {
                bids.remove(0);
                if bids.is_empty() {
                    self.bids.remove(&bid_price);
                }
            }
            if remove_ask {
                asks.remove(0);
                if asks.is_empty() {
                    self.asks.remove(&ask_price);
                }
            }

            // Handle FillAndKill orders that remain unmatched
            if !self.bids.is_empty() {
                let (_, bids) = self.bids.iter().next_back().unwrap();
                let order = &bids[0];
                if order.borrow().get_order_type() == OrderType::FillAndKill {
                    let order_id = order.borrow().get_order_id();
                    self.cancel_order(order_id);
                }
            }
            if !self.asks.is_empty() {
                let (_, asks) = self.asks.iter().next().unwrap();
                let order = &asks[0];
                if order.borrow().get_order_type() == OrderType::FillAndKill {
                    let order_id = order.borrow().get_order_id();
                    self.cancel_order(order_id);
                }
            }

            // If either side is empty, break
            if self.bids.is_empty() || self.asks.is_empty() {
                break;
            }
        }
        trades
    }
    fn prune_gfd_orders(&mut self) {
        todo!();
    }
}

impl Drop for Orderbook{
    fn drop(&mut self){
        self.shutdown.store(true, Ordering::Release);
        self.shutdown_condition_variable.notify_one();

        if let Some(handle) = self.orders_prune_thread.take() {
        handle.join().expect("Failed to join orders_prune_thread");
        }
    }
}
        


/// Tests:

//Each test implicitly assumes a working match_orders() functionality
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_orderbook_new(){
        let orderbook = Orderbook::new(BTreeMap::new(), BTreeMap::new());
        assert_eq!(orderbook.size(), 0)
    }

    #[test]
    fn test_orderbook_add_order(){
        let mut orderbook = Orderbook::new(BTreeMap::new(), BTreeMap::new());
        orderbook.add_order(Order::new(OrderType::GoodTillCancel, 1, Side::Buy, 100, 10));
        orderbook.add_order(Order::new(OrderType::GoodTillCancel, 2, Side::Buy, 100, 10));
        orderbook.add_order(Order::new(OrderType::GoodTillCancel, 3, Side::Buy, 100, 10));
        
        assert_eq!(orderbook.size(), 3);
    }

    #[test]
    fn test_orderbook_cancel_order(){
        let mut orderbook = Orderbook::new(BTreeMap::new(), BTreeMap::new());

        orderbook.add_order(Order::new(OrderType::GoodTillCancel, 1, Side::Buy, 100, 10));
        orderbook.add_order(Order::new(OrderType::GoodTillCancel, 2, Side::Buy, 100, 10));
        orderbook.add_order(Order::new(OrderType::GoodTillCancel, 3, Side::Buy, 100, 10));
        orderbook.cancel_order(1);
        orderbook.cancel_order(2);
        orderbook.cancel_order(3);

        assert_eq!(orderbook.size(), 0);
    }

    #[test]
    fn test_order_modify_order(){
        let mut orderbook = Orderbook::new(BTreeMap::new(),BTreeMap::new());
        orderbook.add_order(Order::new(OrderType::GoodTillCancel, 1, Side::Buy, 100, 10));
        orderbook.add_order(Order::new(OrderType::GoodTillCancel, 2, Side::Buy, 100, 10));

        //create modification
        let order_mod = OrderModify::new(2, Side::Sell, 100, 10);

        //should match and fill order with id 1
        orderbook.modify_order(order_mod);
        assert_eq!(orderbook.size(), 0);
    }

    #[test]
    fn test_orderbook_will_cancel_fnk(){
        let mut orderbook = Orderbook::new(BTreeMap::new(),BTreeMap::new());

        // match should completely fill
        orderbook.add_order(Order::new(OrderType::GoodTillCancel, 2, Side::Sell, 100, 10));
        orderbook.add_order(Order::new(OrderType::FillAndKill, 1, Side::Buy, 100, 10));
        
        
        //Unmatched F&K (should cancel)
        orderbook.add_order(Order::new(OrderType::GoodTillCancel, 3, Side:: Buy, 250, 5));
        orderbook.add_order(Order::new(OrderType::FillAndKill, 4, Side::Buy, 100, 10));

        assert_eq!(orderbook.size(), 1);

    }

    #[test]
    fn test_orderbook_wont_match(){
        let mut ob1 = Orderbook::new(BTreeMap::new(),BTreeMap::new());
        let mut ob2 = Orderbook::new(BTreeMap::new(),BTreeMap::new());
        

        //Same side
        ob1.add_order(Order::new(OrderType::GoodTillCancel, 1, Side::Buy, 1, 1));
        ob1.add_order(Order::new(OrderType::GoodTillCancel, 2, Side::Buy, 1, 1));

        //Ask higher than bid
        ob2.add_order(Order::new(OrderType::GoodTillCancel, 1, Side::Buy, 1, 1));
        ob2.add_order(Order::new(OrderType::GoodTillCancel, 2, Side::Sell, 2, 1));
        
        assert_eq!(ob1.size(), ob2.size());

    }
    

}