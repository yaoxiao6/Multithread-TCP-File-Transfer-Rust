#![feature(catch_panic)]

extern crate encoding;
extern crate console;

use encoding::{Encoding, EncoderTrap};
use encoding::all::ASCII;
use std::error;
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::io;
use std::io::prelude::*;
use std::io::BufWriter;
use std::io::{Read, Write, Result};
use std::str;
use std::fs::File;
use std::fs;

use console::{Term, style};

/*
How my protocol works:
- Both client and server communicate using an 8 byte buffer
- Upon connection, client will attempt to send a message
- Client calculates message size, sends size to server
- Server catches message size and loops in order to assemble message
*/

fn handle_client(mut stream: TcpStream) -> Result<String> {
    loop {
        //buffer (8 bytes)
        let mut buf = [0u8; 8];

        //read message size:
        stream.read(&mut buf).unwrap();

        //interpret the buffer contents into a string slice
        let msg_len_slice: &str = str::from_utf8(&mut buf).unwrap(); //string slice
        let mut msg_len_str = msg_len_slice.to_string(); //convert slice to string

        let mut numeric_chars = 0;
        for c in msg_len_str.chars() {
            if c.is_numeric() == true {
                numeric_chars = numeric_chars + 1;
                }
        }    
        //shrink:
        msg_len_str.truncate(numeric_chars);
        println!("receiving {} bytes", msg_len_str);

        //send message size ACK:
        let ack_str = "ACK".to_string();
        let mut ack_bytes = ASCII.encode(&ack_str, EncoderTrap::Strict).map_err(|x| x.into_owned()).unwrap();
        ack_bytes.push('\r' as u8);
        stream.write_all(&ack_bytes).unwrap();

        //prepare to receive message content:
        let mut remaining_data = msg_len_str.parse::<i32>().unwrap();
        let mut accumulator: String = String::new();
        let mut r = [0u8; 8]; //8 byte buffer

        while remaining_data != 0 {
            if remaining_data >= 8 //(fit or big slab)
            {
                let slab = stream.read(&mut r);
                match slab {
                    Ok(n) => {
                        let r_slice = str::from_utf8(&mut r).unwrap(); //string slice
                        accumulator.push_str(r_slice);
                        println!("wrote {} bytes", n);
                        remaining_data = remaining_data - n as i32;
                    }
                    _ => {}
                }
            } 
            else { //(small slab), shrink
                let slab = stream.read(&mut r);
                match slab {
                    Ok(n) => {
                        let s_slice = str::from_utf8(&mut r).unwrap(); //string slice
                        let mut s_str = s_slice.to_string(); //convert slice to string
                        s_str.truncate(n); //shrink
                        accumulator.push_str(&s_str);
                        println!("wrote {} bytes", n);
                        remaining_data = remaining_data - n as i32;
                    }
                    _ => {}
                }
            }

        }

        //format and output response:
        let index = accumulator.rfind('\r').unwrap();
        format!("{:?}", accumulator.split_off(index));
        println!("{:?}", accumulator);

        if accumulator == "ls-remote"
        {
            let mut ls_bytes = ASCII.encode("", EncoderTrap::Strict).map_err(|x| x.into_owned()).unwrap();

            //send files to client
            println!("{}", style("Local files (/shared)").magenta());
            for entry in fs::read_dir("./src/shared").unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if !path.is_dir() {
                    //clean path from file name:
                    let mut fullpath = String::from(entry.path().to_string_lossy());
                    let filename = String::from(str::replace(&fullpath, "./src/shared", ""));
                    if filename.starts_with("\\"){
                        let trimmed = &filename[1..];

                        let mut file = File::open(fullpath).unwrap();
                        let file_size = file.metadata().unwrap().len();

                        println!("{}  [{:?} bytes]", style(trimmed).green(), style(file_size).cyan());
                        //format data:
                        let partial = format!("{}  [{:?} bytes]", trimmed, file_size);
                        //println!("{:?}", partial);
                        for c in partial.chars()
                        {
                            //load the buffer
                            ls_bytes.push(c as u8);
                        }
                        ls_bytes.push('\n' as u8);

                    }
                    else {
                        // in case not running in windows, file path might work differently
                        //let metadata = fs::metadata(filename);
                        println!("file: {:?}", filename);
                    }
                }
            }
            //wrap the buffer
            let l = ls_bytes.len();
            ls_bytes[l - 1 ] = '\r' as u8;
            //ls_bytes.push('\r' as u8);

            //check if everything is working okay
            //we use these braces to limit the scope of the mutable borrow occuring on line 142:
            {
                let mut slice: &str = str::from_utf8(&mut ls_bytes).unwrap();
                let mut slice_str = slice.to_string(); //convert slice to string
                println!("{}", slice_str);

                //calculate buffer size:
                let length = slice_str.len();
                //convert it to bytes
                let ls_bytes_size = ASCII.encode(&length.to_string(), EncoderTrap::Strict).map_err(|x| x.into_owned()).unwrap();
                //send buffer size:
                stream.write_all(&ls_bytes_size).unwrap();
            }

            //receive ack
            let mut ack_buf = [0u8; 8];
            stream.read(&mut ack_buf).unwrap();
            let ack_slice: &str = str::from_utf8(&mut ack_buf).unwrap(); // string slice
            let mut ack_str = ack_slice.to_string(); //convert slice to string
            let index: usize = ack_str.rfind('\r').unwrap();
            format!("{:?}", ack_str.split_off(index));
            if ack_str == "ACK" {
                println!("received ACK from client");
            }

            //send buffer itself
            stream.write_all(&ls_bytes).unwrap();

        }
    }
}

fn main() {
    let addr = "127.0.0.1:5555";
    let listener = TcpListener::bind(addr).unwrap();
    println!("Listening on addr: {}", style(addr).yellow());
    for stream in listener.incoming() {
        let stream = stream.unwrap();
        thread::Builder::new().name(stream.peer_addr().unwrap().to_string())
        .spawn(move || 
        {
            println!("new client [{}] connected", style(stream.peer_addr().unwrap().to_string()).green());
            let h = handle_client(stream);
        });
    }
}