use std::collections::HashSet;
use std::io::Write;

mod parser;
mod records;
mod serializer;

use records::*;

fn main() {
    let matches = clap::App::new(clap::crate_name!())
        .version(clap::crate_version!())
        .author(clap::crate_authors!())
        .arg(
            clap::Arg::with_name("FILE")
                .help("The input file to decode")
                .required(true),
        )
        .get_matches();

    let file = matches.value_of("FILE").unwrap();
    let bytes = std::fs::read(file).unwrap();

    let mut rec = parser::parse(&bytes).unwrap();

    let mut upgrades_to_add = HashSet::new();
    upgrades_to_add.insert("Hero_Class_Infantry");
    upgrades_to_add.insert("Hero_Class_Pikemen");
    upgrades_to_add.insert("Hero_Class_Archers");

    upgrades_to_add.insert("Hero_Upgrade_PikeCharge");
    upgrades_to_add.insert("Hero_Upgrade_Plunge_Attack");
    upgrades_to_add.insert("Hero_Upgrade_ArcheryFocus");

    upgrades_to_add.insert("Hero_Upgrade_Bomb");
    upgrades_to_add.insert("Hero_Upgrade_Horn");
    upgrades_to_add.insert("Hero_Upgrade_Warhammer");
    upgrades_to_add.insert("Hero_Upgrade_Mine");
    upgrades_to_add.insert("Hero_Upgrade_Size");
    upgrades_to_add.insert("Hero_Upgrade_Grail");
    upgrades_to_add.insert("Hero_Upgrade_PhilosophersStone");
    upgrades_to_add.insert("Hero_Upgrade_Cornucopia");

    upgrades_to_add.insert("Hero_Trait_Sturdy");        // Sure-Footed
    upgrades_to_add.insert("Hero_Trait_Fast");          // Fleet of Foot
    upgrades_to_add.insert("Hero_Trait_CheaperSkills"); // Skillful
    upgrades_to_add.insert("Hero_Trait_SharpWeapons");  // Sharp Weapons
    upgrades_to_add.insert("Hero_Trait_FastReplenish"); // Rousing Speeches
    upgrades_to_add.insert("Hero_Trait_CheaperItems");  // Collector
    upgrades_to_add.insert("Hero_Trait_ExtraArmor");    // Ironskin
    upgrades_to_add.insert("Hero_Trait_ShortCooldown"); // Energetic
    upgrades_to_add.insert("Hero_Trait_BluntWeapons");  // Heavy Weapons ?
    upgrades_to_add.insert("Hero_Trait_ExtraUnit");     // Popular
    upgrades_to_add.insert("Hero_Trait_ExtraUses");     // Heavy Load
    upgrades_to_add.insert("Hero_Trait_Giant");         // Mountain
                                                        // Fearless


    let mut upgrade_entries_to_update = Vec::new();
    let mut upgrade_inners_to_update = Vec::new();
    let mut upgrade_entry_class_id = None;
    let mut upgrade_inner_class_id = None;

    let user_save = rec.records[&rec.root_id].as_class();
    let inventory = rec.class_member_deref(user_save, "inventory").as_class();
    let upgrades_id = *rec.class_member(inventory, "upgrades").as_reference();
    let upgrades = rec.records[&upgrades_id].as_class();
    let length = rec.class_member(upgrades, "_size").as_i32() as usize;
    let length_index = rec.class_member_index(upgrades, "_size");
    let items_id = *rec.class_member(upgrades, "_items").as_reference();
    let items = rec.records[&items_id].as_binary_array();

    for item in &items[..length] {
        let entry = rec.records[item.as_reference()].as_class();
        let upgrade = rec.class_member_deref(entry, "upgrade").as_class();
        let name_id = rec.class_member(upgrade, "name").as_reference();
        let name = rec.records[name_id].as_string();
        if !upgrades_to_add.remove(name) {
            println!("Unknown upgrade: '{}'", name);
        }
        if can_be_starting(name) {
            upgrade_entries_to_update.push(*item.as_reference());
        }
        upgrade_inners_to_update.push(*rec.class_member(entry, "upgrade").as_reference());
        upgrade_entry_class_id = Some(entry.class_type_id);
        upgrade_inner_class_id = Some(upgrade.class_type_id);
    }

    for id in upgrade_entries_to_update {
        let entry = rec.records[&id].as_class();
        let is_starting_index = rec.class_member_index(entry, "isStarting");
        let entry = rec.records.get_mut(&id).unwrap().as_class_mut();
        entry.members[is_starting_index] = Member::Primitive(Primitive::Boolean(true));
    }

    for id in upgrade_inners_to_update {
        let entry = rec.records[&id].as_class();
        let level_index = rec.class_member_index(entry, "level");
        let entry = rec.records.get_mut(&id).unwrap().as_class_mut();
        entry.members[level_index] = Member::Primitive(Primitive::Int32(2));
    }

    rec.records
        .get_mut(&upgrades_id)
        .unwrap()
        .as_class_mut()
        .members[length_index] =
        Member::Primitive(Primitive::Int32((length + upgrades_to_add.len()) as i32));

    let mut upgrade_entries_to_add = Vec::new();
    let mut next_id = rec.records.keys().max().unwrap() + 1;

    for upgrade_name in upgrades_to_add {
        upgrade_entries_to_add.push(next_id);
        rec.records.insert(
            next_id,
            Record::Class(Class {
                class_type_id: upgrade_entry_class_id.unwrap(),
                members: vec![
                    Member::Reference(next_id + 1),
                    Member::Primitive(Primitive::Boolean(can_be_starting(upgrade_name))),
                    Member::Primitive(Primitive::Boolean(true)),
                ],
            }),
        );
        rec.records.insert(
            next_id + 1,
            Record::Class(Class {
                class_type_id: upgrade_inner_class_id.unwrap(),
                members: vec![
                    Member::Reference(next_id + 2),
                    Member::Primitive(Primitive::Int32(2)),
                ],
            }),
        );
        rec.records
            .insert(next_id + 2, Record::String(upgrade_name.into()));
        next_id += 3;
    }

    let mut next_index = length;
    let items = rec
        .records
        .get_mut(&items_id)
        .unwrap()
        .as_binary_array_mut();

    for id in upgrade_entries_to_add {
        if next_index < items.len() {
            items[next_index] = Member::Reference(id);
        } else {
            items.push(Member::Reference(id));
        }
        next_index += 1;
    }

    let output = serializer::serialize(&rec);
    let mut file = std::fs::File::create(format!("{}.new", file)).unwrap();
    file.write_all(&output).unwrap();
}

fn can_be_starting(s: &str) -> bool {
    match s {
        "Hero_Class_Infantry"
        | "Hero_Class_Pikemen"
        | "Hero_Class_Archers"
        | "Hero_Upgrade_PikeCharge"
        | "Hero_Upgrade_Plunge_Attack"
        | "Hero_Upgrade_ArcheryFocus" => false,
        _ => true,
    }
}
