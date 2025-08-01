mod orderbook;
use std::{
    rc::Rc,
    cell::RefCell,
    collections::{BTreeMap, HashMap}
};
use crate::orderbook::{Orderbook, Order, OrderType, Side};


fn main() {
    let orderbook = Orderbook::new(BTreeMap::new(), BTreeMap::new());

        orderbook.add_order(Order::new(OrderType::GoodTillCancel, 1, Side::Buy, 100, 10));
        orderbook.add_order(Order::new(OrderType::GoodTillCancel, 2, Side::Buy, 100, 10));
        // orderbook.add_order(Order::new_shared(OrderType::GoodTillCancel, 3, Side::Buy, 100, 10));
        println!("BEFORE CANCELS");
        println!("{:#?}", orderbook);
        
        orderbook.cancel_order(1);
        orderbook.cancel_order(2);
        // orderbook.cancel_order(3);
        println!("AFTER CANCELS");
        println!("{:#?}", orderbook);
    }   
    // // Create an empty orderbook
    // let mut orderbook = Orderbook::new(BTreeMap::new(), BTreeMap::new());

    // // Add several buy and sell orders
    // let orders = vec![
    //     Rc::new(RefCell::new(Order::new(OrderType::GoodTillCancel, 1, Side::Buy, 100, 10))),
    //     Rc::new(RefCell::new(Order::new(OrderType::GoodTillCancel, 2, Side::Sell, 100, 10))),
    //     Rc::new(RefCell::new(Order::new(OrderType::GoodTillCancel, 3, Side::Buy, 101, 5))),
    //     Rc::new(RefCell::new(Order::new(OrderType::GoodTillCancel, 4, Side::Sell, 99, 7))),
    //     Rc::new(RefCell::new(Order::new(OrderType::GoodTillCancel, 5, Side::Buy, 98, 8))),
    //     Rc::new(RefCell::new(Order::new(OrderType::GoodTillCancel, 6, Side::Sell, 102, 6))),
    // ];

    // let mut prev_size = orderbook.size();

    // for order in &orders {
    //     orderbook.add_order(order.clone());
    //     let o = order.borrow();
    //     let new_size = orderbook.size();
    //     println!(
    //         "Added {:?} Order: ID:{}, Qty:{}, Price:{}, Book Size:{}",
    //         o.get_side(),
    //         o.get_order_id(),
    //         o.get_initial_quantity(),
    //         o.get_price(),
    //         new_size
    //     );
    //     if new_size < prev_size {
    //         println!("Orders Matched!");
    //     }
    //     prev_size = new_size;
    // }

