//! vmstat
//! # vmstat
//!
//! Vmstat collects stat infomation from virsh. Mainly for OpenStack admin.


/*
TODO: forllow this guide
https://github.com/rust-lang/style-team/blob/master/guide/guide.md
*/

use clap::Parser;
use ssh2::Session;
use std::env;
use std::io::Read;
use std::net::TcpStream;
#[macro_use]
extern crate prettytable;
use prettytable::format;
use prettytable::Table;
use prettytable::{color, Attr};

#[derive(Parser, Debug)]
struct Args {
    #[arg(value_name = "host", help = "Target compute node name, or IP address")]
    pos_arg: String,
}

/// Status information set for each virsh domain
struct VMStats {
    domain: String,
    instance: String,
    cpu: i64,
    mem_cur: i64,
    mem_max: i64,
    io: i64,
    net: i64,
    allocation: i64,
    capacity: i64,
}

/// SSH sender impletemted with SSH2
fn run_ssh(user: &str, host: &str, cmd: &str) -> String {
    let tcp = TcpStream::connect(host).expect("Failed to connect");
    let mut ssn = Session::new().expect("Failed to create a new session");
    ssn.set_tcp_stream(tcp);
    ssn.handshake().expect("Failed at TCP handshake");
    ssn.userauth_agent(user)
        .expect("Failed to have user auth agent");
    assert!(ssn.authenticated());

    let mut channel = ssn.channel_session().expect("Failed to create a channel");
    channel
        .exec(cmd)
        .expect("Failed to run command through SSH");
    let mut result = String::new();
    channel
        .read_to_string(&mut result)
        .expect("Failed to read the result");
    return result;
}

fn main() {
    const GIGA: i64 = 1000000000;
    const MEGA: i64 = 1000000;
    let mut vmstats_list: Vec<VMStats> = vec![];

    // Get target node, port, user
    let args = Args::parse();
    let host: String = format!("{}:22", args.pos_arg);
    let user: String = env::var("USER").expect("$USER is not set");

    // Run 'virsh domstats' in target node
    let domstats = run_ssh( &user,
                            &host,
                            "sudo virsh domstats --cpu-total --balloon --interface --block \
                            | grep -e Domain: -e cpu.time -e balloon -e bytes -e allocation -e capacity");

    let mut index = 0;
    let mut cpu_top: i64 = 0;
    let mut io_top: i64 = 0;
    let mut net_top: i64 = 0;
    let mut domain_list: String = "".to_string();

    // Collect status from each domain(instance)
    for buff in domstats.lines() {
        let line = buff.trim();

        // Ask instance name if line contains domain name
        if line.contains("Domain: ") {
            let domain: Vec<&str> = line.split('\'').collect();
            let vmstats = VMStats {
                domain: domain[1].to_string(),
                instance: "".to_string(),
                cpu: 0,
                mem_cur: 0,
                mem_max: 0,
                io: 0,
                net: 0,
                allocation: 0,
                capacity: 0,
            };
            vmstats_list.push(vmstats);
            index = vmstats_list.len() - 1;
            domain_list += format!(" {}", domain[1]).as_str();
            continue;
        }

        // Sprit A.B.C=xxxx
        let keyvalue: Vec<&str> = line.split('=').collect();
        let key: Vec<&str> = keyvalue[0].split('.').collect();
        let value = keyvalue[1].parse::<i64>().unwrap();

        // Collect data for cpu, memory, block, net
        match key[0] {
            "cpu" => {
                if *key.last().unwrap() == "time" {
                    vmstats_list[index].cpu = value;
                }
            }
            "balloon" => match *key.last().unwrap() {
                "current" => vmstats_list[index].mem_cur = value,
                "maximum" => vmstats_list[index].mem_max = value,
                _ => (),
            },
            "block" => match *key.last().unwrap() {
                "bytes" => vmstats_list[index].io += value,
                "allocation" => vmstats_list[index].allocation = value,
                "capacity" => vmstats_list[index].capacity = value,
                _ => (),
            },
            "net" => {
                if *key.last().unwrap() == "bytes" {
                    vmstats_list[index].net += value;
                }
            }
            _ => (),
        }
    }

    // Get instance name from domain name
    let cmd: String = format!(
        "{} {} {} {} {} {}",
        "for DOMAIN in",
        domain_list,
        "; do ",
        "sudo virsh dumpxml ${DOMAIN}",
        "| grep nova:name | sed -r 's/<nova:name>(.*)<\\/nova:name>/\\1/';",
        "done;"
    );
    let instances = run_ssh(user.as_str(), host.as_str(), &cmd);
    let mut index = 0;
    for instance in instances.lines() {
        let instance = instance.trim();
        vmstats_list[index].instance = instance.to_string();
        index += 1;
    }

    // Print table
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
    table.set_titles(
        row![bc => "Domain", "Instance", "CPU(G)", "MEM(G)", "I/O(G)","NET(G)", "Disk(G)"],
    );
    for vmstats in &vmstats_list {
        table.add_row(row![
            vmstats.domain,
            vmstats.instance,
            r->(vmstats.cpu/GIGA).to_string(),
            r->format!("{}/{}", (vmstats.mem_cur/MEGA).to_string(),(vmstats.mem_max/MEGA).to_string()),
            r->(vmstats.io/GIGA).to_string(),
            r->(vmstats.net/GIGA).to_string(),
            r->format!("{}/{}", (vmstats.allocation/GIGA).to_string(),(vmstats.capacity/GIGA).to_string()),
        ]);

        // Record top resource consumer
        if vmstats.cpu > cpu_top {
            cpu_top = vmstats.cpu
        };
        if vmstats.io > io_top {
            io_top = vmstats.io
        };
        if vmstats.net > net_top {
            net_top = vmstats.net
        };
    }

    // Coloring red for top resource comsumer
    table.column_iter_mut(2).for_each(|column| {
        if column.get_content() == (cpu_top / GIGA).to_string() {
            column.style(Attr::ForegroundColor(color::RED));
        }
    });
    table.column_iter_mut(4).for_each(|column| {
        if column.get_content() == (io_top / GIGA).to_string() {
            column.style(Attr::ForegroundColor(color::RED));
        }
    });
    table.column_iter_mut(5).for_each(|column| {
        if column.get_content() == (net_top / GIGA).to_string() {
            column.style(Attr::ForegroundColor(color::RED));
        }
    });
    table.printstd();
}
