module object_crawler_test::main;

//! This Move module defines an object with various fields and types that can
//! be used to test the object crawler.

use std::ascii::String as AsciiString;

use sui::object_bag::{Self, ObjectBag};
use sui::object_table::{Self, ObjectTable};
use sui::table::{Self, Table};
use sui::table_vec::{Self, TableVec};
use sui::linked_table::{Self, LinkedTable};
use sui::vec_map::{Self, VecMap};
use sui::vec_set::{Self, VecSet};
use sui::bag::{Self, Bag};
use sui::transfer::public_share_object;

public struct Guy has key, store {
    id: UID,

    name: AsciiString,
    age: u8,

    hobbies: VecSet<AsciiString>,
    groups: VecMap<Name, vector<Name>>,

    chair: Table<Name, Name>,
    timetable: ObjectTable<Name, Value>,
    friends: ObjectBag,
    bag: Bag,
    heterogeneous: Bag,
    sequence: TableVec<Name>,
    linked_table: LinkedTable<Name, Name>,
}

public struct Name has copy, drop, store {
    name: AsciiString,
}

public struct AnotherName has copy, drop, store {
    another_name: AsciiString,
}

public struct Value has key, store {
    id: UID,
    value: Name,
    pouch: ObjectBag,
}

public struct PlainValue has key, store {
    id: UID,
    value: vector<u8>,
}

public struct AnotherPlainValue has key, store {
    id: UID,
    another_value: vector<u8>,
}

fun init(ctx: &mut TxContext) {
    let guy_id = object::new(ctx);
    let name = b"John Doe".to_ascii_string();
    let age = 30;

    let mut hobbies = vec_set::empty();

    hobbies.insert( b"Reading".to_ascii_string());
    hobbies.insert(b"Swimming".to_ascii_string());

    let mut groups = vec_map::empty();

    let group_1_name = Name { name: b"Book Club".to_ascii_string() };
    let mut group_1_members = vector::empty();
    group_1_members.push_back(Name { name: b"Alice".to_ascii_string() });
    group_1_members.push_back(Name { name: b"Bob".to_ascii_string() });

    let group_2_name = Name { name: b"Swimming Club".to_ascii_string() };
    let mut group_2_members = vector::empty();
    group_2_members.push_back(Name { name: b"Charlie".to_ascii_string() });
    group_2_members.push_back(Name { name: b"David".to_ascii_string() });

    groups.insert(group_1_name, group_1_members);
    groups.insert(group_2_name, group_2_members);

    let mut chair = table::new(ctx);
    chair.add(Name { name: b"Chairman".to_ascii_string() }, Name { name: b"John Doe".to_ascii_string() });
    chair.add(Name { name: b"Vice Chairman".to_ascii_string() }, Name { name: b"Alice".to_ascii_string() });

    let mut timetable = object_table::new(ctx);

    let mut pouch = object_bag::new(ctx);
    let pouch_value = PlainValue {
        id: object::new(ctx),
        value: b"Pouch Data",
    };
    pouch.add( Name { name: b"Pouch Item".to_ascii_string() }, pouch_value);
    let timetable_1_value = Value {
        id: object::new(ctx),
        value: Name { name: b"Meeting".to_ascii_string() },
        pouch,
    };

    let mut pouch = object_bag::new(ctx);
    let pouch_value = PlainValue {
        id: object::new(ctx),
        value: b"MOREDATA15",
    };
    pouch.add( Name { name: b"Pouch Code".to_ascii_string() }, pouch_value);
    let timetable_2_value = Value {
        id: object::new(ctx),
        value: Name { name: b"Code Review".to_ascii_string() },
        pouch,
    };

    timetable.add( Name { name: b"Monday".to_ascii_string() }, timetable_1_value);
    timetable.add( Name { name: b"Tuesday".to_ascii_string() }, timetable_2_value);

    let mut friends = object_bag::new(ctx);
    let friend_1_value = PlainValue {
        id: object::new(ctx),
        value: b"Never Seen",
    };
    let friend_2_value = PlainValue {
        id: object::new(ctx),
        value: b"Definitely Imagination",
    };
    friends.add(Name { name: b"Charlie".to_ascii_string() }, friend_1_value);
    friends.add(Name { name: b"David".to_ascii_string() }, friend_2_value);

    let mut bag = bag::new(ctx);
    bag.add(Name { name: b"Bag Item".to_ascii_string() }, PlainValue {
        id: object::new(ctx),
        value: b"Bag Data",
    });
    bag.add(Name { name: b"Bag Item 2".to_ascii_string() }, PlainValue {
        id: object::new(ctx),
        value: b"Bag Data 2",
    });

    let mut heterogeneous = bag::new(ctx);
    heterogeneous.add(Name { name: b"Bag Item".to_ascii_string() }, PlainValue {
        id: object::new(ctx),
        value: b"Bag Data",
    });
    heterogeneous.add(Name { name: b"Another Bag Item".to_ascii_string() }, AnotherPlainValue {
        id: object::new(ctx),
        another_value: b"Another Bag Data",
    });

    let mut sequence = table_vec::empty(ctx);
    table_vec::push_back(&mut sequence, Name { name: b"First".to_ascii_string() });
    table_vec::push_back(&mut sequence, Name { name: b"Second".to_ascii_string() });
    table_vec::push_back(&mut sequence, Name { name: b"Third".to_ascii_string() });

    let mut linked_table = linked_table::new(ctx);
    linked_table.push_back(Name { name: b"Key 1".to_ascii_string() }, Name { name: b"Value 1".to_ascii_string() });

    let guy = Guy {
        id: guy_id,
        name,
        age,
        hobbies,
        groups,
        chair,
        timetable,
        friends,
        bag,
        heterogeneous,
        sequence,
        linked_table,
    };

    public_share_object(guy);
}
