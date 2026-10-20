use crate::{BinaryConfig, BinaryType, BinaryValue, IdCrc};

use std::collections::{HashMap, VecDeque};

impl BinaryConfig {
    pub fn parse_json(root: &serde_json::Value) -> Self {
        use serde_json::Value;

        let len = count_elements(root);
        let mut buffer = vec![0_u8; len * std::mem::size_of::<BinaryValue>()];

        let mut idx = 0;
        let mut names: HashMap<&str, (u64, IdCrc)> = HashMap::new();

        let mut queue = VecDeque::new();
        queue.push_back((root, 0, IdCrc::default()));

        while let Some((value, id, id_crc)) = queue.pop_front() {
            let value = match value {
                Value::Null => unreachable!(),
                Value::Bool(data) => {
                    let data: u8 = (*data).into();

                    BinaryValue {
                        data: data as u64,
                        id,
                        id_crc,
                        type_: BinaryType::Boolean,
                        count: 0x1,
                    }
                }
                Value::Number(data) => {
                    if let Some(data) = data.as_i64() {
                        let data: i32 = data.try_into().unwrap();
                        BinaryValue {
                            data: data as u64,
                            id,
                            id_crc,
                            type_: BinaryType::Integer,
                            count: 0x4,
                        }
                    } else if let Some(data) = data.as_f64() {
                        let data: u32 = (data as f32).to_bits();

                        BinaryValue {
                            data: data as u64,
                            id,
                            id_crc,
                            type_: BinaryType::Float,
                            count: 0x4,
                        }
                    } else {
                        unreachable!()
                    }
                }

                // @NOTE: We are mixing values with indexes, which we might not want to do!
                // Survarium devs did it in the same way, so whatever.
                Value::String(data) => {
                    let offset = buffer.len() as u64;
                    buffer.extend(data.as_bytes());
                    buffer.push(0);

                    let count: u16 = (data.len() + 1).try_into().unwrap();
                    let data = offset;

                    BinaryValue {
                        data,
                        id,
                        id_crc,
                        type_: BinaryType::String,
                        count,
                    }
                }
                Value::Array(values) => {
                    let data = get_offset(idx, queue.len());
                    let count = values.len().try_into().unwrap();

                    queue.extend(
                        values
                            .iter()
                            .enumerate()
                            .map(|(id, v)| (v, id as u64, IdCrc::default())),
                    );

                    BinaryValue {
                        data,
                        id,
                        id_crc,
                        type_: BinaryType::TableIndexed,
                        count,
                    }
                }
                Value::Object(values) => {
                    let data = get_offset(idx, queue.len());
                    let count = values.len().try_into().unwrap();

                    let mut values = values
                        .into_iter()
                        .map(|(k, v)| {
                            let (id, id_crc) = *names.entry(k).or_insert_with_key(|name| {
                                let offset = buffer.len() as u64;
                                buffer.extend(name.as_bytes());
                                buffer.push(0);
                                let crc = IdCrc::get_hash(k);

                                (offset, crc)
                            });

                            (v, id, id_crc)
                        })
                        .collect::<Vec<_>>();
                    values.sort_by_key(|v| v.2.get());

                    queue.extend(values);

                    BinaryValue {
                        data,
                        id,
                        id_crc,
                        type_: BinaryType::TableNamed,
                        count,
                    }
                }
            };

            write_value(&mut buffer, idx, value);
            idx += 1;
        }

        Self(buffer)
    }
}

fn count_elements(input: &serde_json::Value) -> usize {
    let inner_items = match input {
        serde_json::Value::Array(values) => values.iter().map(count_elements).sum(),
        serde_json::Value::Object(values) => values.values().map(count_elements).sum(),
        _ => 0,
    };
    inner_items + 1
}

fn get_offset(idx: usize, queue_len: usize) -> u64 {
    ((idx + queue_len + 1) * BinaryValue::SIZE) as u64
}

fn write_value(buffer: &mut [u8], idx: usize, value: BinaryValue) {
    buffer[idx * BinaryValue::SIZE..(idx + 1) * BinaryValue::SIZE]
        .copy_from_slice(value.as_bytes());
}
