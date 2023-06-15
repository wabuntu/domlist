use ssh2::Session;
use std::io::{Read};
use std::env;
use std::net::TcpStream;
// use std::thread;
// use tokio::time;
#[macro_use] extern crate prettytable;
use prettytable::{Table};
use prettytable::format;
use prettytable::{Attr, color};
// use colored::Colorize;


fn run_ssh(user: &str, host: &str, cmd: &str) -> String {

    let tcp = TcpStream::connect(host).expect("Failed to connect");
    let mut ssn = Session::new().expect("Failed to create a new session");
    ssn.set_tcp_stream(tcp);
    ssn.handshake().expect("Failed at TCP handshake");
    ssn.userauth_agent(user).expect("Failed to have user auth agent");
    assert!(ssn.authenticated());

    let mut channel = ssn.channel_session().expect("Failed to create a channel");
    channel.exec(cmd).expect("Failed to run command through SSH");
    let mut result = String::new();
    channel.read_to_string(&mut result).expect("Failed to read the result");

//    channel.send_eof().expect("Failed to send EOF");
//    channel.wait_eof().unwrap();
//    channel.close().expect("Failed to close");
//    channel.wait_close().unwrap();

    return result;
}

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
fn main() {

    const GIGA : i64 = 1000000000;
    const MEGA : i64 = 1000000;
    let mut vmstats_list: Vec<VMStats> = vec![];

    // Get target node and user
    // TODO Use Clap crate
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Target node is required.");
        return;
    }
    let host: String = format!("{}:22", args[1]);
    let user: String = env::var("USER").expect("$USER is not set");

    // Run 'virsh domstats' in target node
    let domstats = run_ssh( &user,
                            &host,
                            "sudo virsh domstats --cpu-total --balloon --interface --block \
                            | grep -e Domain: -e cpu.time -e balloon -e bytes -e allocation -e capacity");
    //print_table_top();

/* Sample output
Domain: 'vm-001'
  cpu.time=287227998002
  cpu.user=3420000000
  cpu.system=47170000000
  balloon.current=16777216
  balloon.maximum=16777216
  net.count=1
  net.0.name=tap503eedb1-62
  net.0.rx.bytes=205574278751191
  net.0.rx.pkts=21214611866
  net.0.rx.errs=0
  net.0.rx.drop=580571
  net.0.tx.bytes=16044063043681
  net.0.tx.pkts=19474726360
  net.0.tx.errs=0
  net.0.tx.drop=0
  block.count=6
  block.0.name=vda
  block.0.path=/dev/drbd1021
  block.0.rd.reqs=1222644
  block.0.rd.bytes=22719836672
  block.0.rd.times=995287307927
  block.0.wr.reqs=56389817
  block.0.wr.bytes=1167926535168
  block.0.wr.times=190253961087617
  block.0.fl.reqs=4379880
  block.0.fl.times=374942116357
  block.0.allocation=21478596608
  block.0.capacity=21478596608
  block.0.physical=21478596608
*/

    let mut index = 0;
    let mut cpu_top: i64 = 0;
    let mut io_top: i64 = 0;
    let mut net_top: i64 = 0;
    let mut cpu_2nd: i64 = 0;
    let mut io_2nd: i64 = 0;
    let mut net_2nd: i64 = 0;

    // Collect status from each domain(instance)
    for buff in domstats.lines() {
        let line = buff.trim();

        // Ask instance name if line contains domain name
        if line.contains("Domain: ") {
            let domain :Vec<&str> = line.split('\'').collect();
            let vmstats = VMStats{ domain: domain[1].to_string(),
                                            instance: "".to_string(),
                                            cpu: 0,
                                            mem_cur: 0,
                                            mem_max: 0,
                                            io: 0,
                                            net: 0,
                                            allocation: 0,
                                            capacity: 0 };
            vmstats_list.push(vmstats);
            index = vmstats_list.len()-1;
            continue;
        }

        let keyvalue: Vec<&str> = line.split('=').collect();
        let key: Vec<&str> = keyvalue[0].split('.').collect();
        let value = keyvalue[1].parse::<i64>().unwrap();

        // Collect data for cpu, block, net
        match key[0] {
            "cpu" => {
                if *key.last().unwrap() == "time" {
                    vmstats_list[index].cpu = value;
                    if vmstats_list[index].cpu > cpu_top {
                        cpu_2nd = cpu_top;
                        cpu_top = vmstats_list[index].cpu
                    };
                }
            },
            "balloon" => {
                match *key.last().unwrap() {
                    "current" => vmstats_list[index].mem_cur = value,
                    "maximum" => vmstats_list[index].mem_max = value,
                    _ => (),
                }
            },
            "block" => {
                match *key.last().unwrap() {
                    "bytes" => vmstats_list[index].io += value,
                    "allocation" => vmstats_list[index].allocation = value,
                    "capacity" => vmstats_list[index].capacity = value,
                    _ => (),
                }
                if vmstats_list[index].io > io_top {
                    io_2nd = io_top;
                    io_top = vmstats_list[index].io
                };
            },
            "net" => {
                if *key.last().unwrap() == "bytes" {
                    vmstats_list[index].net += value;
                    if vmstats_list[index].net > net_top {
                        net_2nd = net_top;
                        net_top = vmstats_list[index].net
                    };
                }
            },
            _ => (),
        }
    }

    // Get instance name from domain name
    let mut domain_list: String = "".to_string();
    for vmstats in &vmstats_list {
        domain_list += format!(" {}", vmstats.domain).as_str();
    }
    let cmd: String = format!("{} {} {} {} {} {}",
                                "for DOMAIN in",
                                domain_list,
                                "; do ",
                                "sudo virsh dumpxml ${DOMAIN}",
                                "| grep nova:name | sed -r 's/<nova:name>(.*)<\\/nova:name>/\\1/';",
                                "done;");
    let instances = run_ssh( user.as_str(), host.as_str(), &cmd);
    let mut index = 0;
    for instance in instances.lines() {
        let instance = instance.trim();
        vmstats_list[index].instance = instance.to_string();
        index += 1;
    }

    //for index in 0..vmstats_list.len() {
        //dbg!(index);
        //thread::scope(|thread_ssh| {
            //thread_ssh.spawn(|| {
        //async {
            //dbg!(index);

            //dbg!(index);
        //}.await;
            //});
        //});
    //}

    // Print table
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
    table.set_titles(row![bc => "Domain", "Instance", "CPU(G)", "MEM(G)", "I/O(G)","NET(G)", "Disk(G)"]);
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
    }

println!("io_second: {}", io_2nd);

    table.column_iter_mut(2).for_each(|column| {
        if column.get_content() == (cpu_2nd/GIGA).to_string() {
            column.style(Attr::ForegroundColor(color::YELLOW));
        }
        if column.get_content() == (cpu_top/GIGA).to_string() {
            column.style(Attr::ForegroundColor(color::RED));
        }
    });
    table.column_iter_mut(4).for_each(|column| {
        if column.get_content() == (io_2nd/GIGA).to_string() {
            column.style(Attr::ForegroundColor(color::YELLOW));
        }
        if column.get_content() == (io_top/GIGA).to_string() {
            column.style(Attr::ForegroundColor(color::RED));
        }
    });
    table.column_iter_mut(5).for_each(|column| {
        if column.get_content() == (net_2nd/GIGA).to_string() {
            column.style(Attr::ForegroundColor(color::YELLOW));
        }
        if column.get_content() == (net_top/GIGA).to_string() {
            column.style(Attr::ForegroundColor(color::RED));
        }
    });
    table.printstd();

}
