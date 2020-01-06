use std::fs::{File};
use std::io::prelude::*;
use std::io::{BufReader,BufWriter};
use bitbit::{BitWriter};
use std::collections::HashMap;
use std::iter::Iterator;
use std::env;
use std::time::{SystemTime};

enum CompressionType {
    Unlimited,
    LimitDepth(u128),
    LimitSize(u128)
}

fn main() {
    let start_time = SystemTime::now();
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Please provide filename argument");
        return;        
    }
    let filename = &args[1];
    
    let mut case = CompressionType::Unlimited;
    let mut dict_limit_parameter = 0u8;
    if args.len() == 3 {
        case = match (&args[2]).parse::<i8>() {
            Ok(number) => {
                if number > 0 {
                    let limit = 2u128.pow(number as u32);
                    CompressionType::LimitDepth(limit)
                } else {
                    dict_limit_parameter = (0 - number) as u8;
                    let limit = 2u128.pow(dict_limit_parameter as u32);
                    CompressionType::LimitSize(limit)
                }
            },
            Err(e) => { 
                println!("Error parsing compression parameter: {}. Using default case: unlimited length", e);
                CompressionType::Unlimited
            }       
        }
    }

    let mut dicts: Vec<Tree> = Vec::new();
    match case {
        CompressionType::LimitSize(dict_len_limit) => {
            dicts = create_multiple_dicts(filename.to_string(), dict_len_limit);
        },
        _ => {
            dicts.push(create_dict_from_file(filename.to_string(), &case));
        }
    };

    let w = File::create(format!("{}.lz", filename)).unwrap();
    let mut buf_writer = BufWriter::new(w);
    let mut bw = BitWriter::new(&mut buf_writer);
    match case {
        CompressionType::LimitSize(_) => {
            bw.write_byte(dict_limit_parameter).unwrap();
        },
        _ => {
            bw.write_byte(0u8).unwrap();
        }
    };
    let mut bits_remainder_to_byte = 0usize;
    for dict in dicts {
        if dict.nodes.len() < 2 {
            continue;
        }
        let first_entry = dict.nodes.get(1).unwrap();
        bw.write_byte(first_entry.value).unwrap();
        for i in 2..dict.nodes.len() {
            let node = dict.nodes.get(i).unwrap();
            let node_idx_bit_length = log2(i-1) as usize;
            bw.write_bits(node.parent_node as u32, node_idx_bit_length).unwrap();
            bw.write_byte(node.value).unwrap();
            bits_remainder_to_byte = (node_idx_bit_length + bits_remainder_to_byte) % 8;
        }
    }
    
    bw.write_bits(0x00, 8 - bits_remainder_to_byte).unwrap();
    buf_writer.flush().unwrap();
    match start_time.elapsed() {
        Ok(elapsed) => {
            println!("Compressed in: {} s", elapsed.as_secs());
        }
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }
}

fn create_dict_from_file(filename: String, case: &CompressionType) -> Tree {
    let file = File::open(filename).unwrap();
    let buf_reader = BufReader::new(file);
    let mut dict = Tree::new();
    let mut current_node = dict.nodes.first().unwrap().clone();
    let mut max_depth = 0u128;
    let mut current_depth = max_depth;

    for byte in buf_reader.bytes() {
        current_depth += 1;
        match byte {
            Ok(value) => {
                if let Some(child_index) = current_node.children.get(&value) {
                    match case {
                        CompressionType::LimitDepth(limit) => {
                            if *limit == current_depth {
                                current_depth = 0;
                                dict.add_node(value, current_node.index);
                                current_node = dict.nodes.first().unwrap().clone();
                            } else {
                                current_node = dict.nodes.get(*child_index).unwrap().clone();
                            }
                        },
                        _ => {
                            current_node = dict.nodes.get(*child_index).unwrap().clone();
                        }
                    }
                } else {
                    if current_depth > max_depth {
                        max_depth = current_depth;
                    }
                    current_depth = 0;
                    dict.add_node(value, current_node.index);
                    current_node = dict.nodes.first().unwrap().clone();
                }
            },
            _ => println!("Error in reading file.")
        }
    }
    if current_depth > 0 {
        let parent = dict.nodes.get(current_node.parent_node).unwrap();
        dict.add_node(current_node.value, parent.index);
    }
    dict
}

fn create_multiple_dicts(filename: String, dict_len_limit: u128) -> Vec<Tree> {
    let mut res = Vec::new();
    let file = File::open(filename).unwrap();
    let buf_reader = BufReader::new(file);
    let mut dict = Tree::new();
    let mut current_node = dict.nodes.first().unwrap().clone();
    let mut current_depth = 0u128;

    for byte in buf_reader.bytes() {
        current_depth += 1;
        match byte {
            Ok(value) => {
                if let Some(child_index) = current_node.children.get(&value) {
                    current_node = dict.nodes.get(*child_index).unwrap().clone();
                } else {
                    current_depth = 0;
                    dict.add_node(value, current_node.index);
                    if (dict.nodes.len() as u128) <= dict_len_limit {
                        current_node = dict.nodes.first().unwrap().clone();
                    } else {
                        res.push(dict.clone());
                        dict = Tree::new();
                        current_node = dict.nodes.first().unwrap().clone();
                    }
                }
            },
            _ => println!("Done.")
        }
    }
    if current_depth > 0 {
        let parent = dict.nodes.get(current_node.parent_node).unwrap();
        dict.add_node(current_node.value, parent.index);
    }
    res.push(dict);
    res
}

fn log2(number: usize) -> u32 {
    64 - number.leading_zeros()
}

#[derive(Clone, Debug)]
struct Tree {
    nodes: Vec<Node>
}

impl Tree {
    pub fn new() -> Tree {
        let mut nodes: Vec<Node> = Vec::new();
        let root_node = Node {
            value: 0,
            index: 0,
            parent_node: 0,
            children: HashMap::new()
        };
        nodes.push(root_node);
        Tree {
            nodes
        }
    }

    pub fn add_node(&mut self, value: u8, parent_node: usize) -> usize {
        let index = self.nodes.len();
        self.nodes.push(Node {
            value,
            index,
            parent_node,
            children: HashMap::new()
        });
        if let Some(pn) = self.nodes.get_mut(parent_node) {
            match (*pn).children.get(&value) {
                None => {(*pn).children.insert(value, index);},
                _ => {}
            }
        }
        index
    }

}

#[derive(Debug, Clone)]
struct Node {
    parent_node: usize,
    children: HashMap<u8, usize>,
    index: usize,
    value: u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adding_node() {
        let mut test_tree = Tree::new();
        test_tree.add_node(1, 0);
        assert_eq!(2, test_tree.nodes.len());
        assert_eq!(1, test_tree.nodes.first().unwrap().children.len());
    }
   
    #[test]
    fn test_log2() {
        assert_eq!(5, log2(23));
        assert_eq!(2, log2(2));
        assert_eq!(3, log2(5));
        assert_eq!(5, log2(16));
    }

}