use ssh2::Session;
use std::io::Read;
use std::env;
use std::net::TcpStream;

fn run_ssh(user: &str, host: &str, cmd: &str) -> String {

    let tcp = TcpStream::connect(host).unwrap();
    let mut ssn = Session::new().unwrap();
    ssn.set_tcp_stream(tcp);
    ssn.handshake().unwrap();
    ssn.userauth_agent(user).unwrap();
    assert!(ssn.authenticated());

    let mut channel = ssn.channel_session().unwrap();
    channel.exec(cmd).unwrap();
    let mut result = String::new();
    channel.read_to_string(&mut result).unwrap();

    channel.send_eof();
    // channel.wait_eof();
    channel.close();
    // channel.wait_close();
    //println!("{}", channel.exit_status().unwrap());

    return result;
}

fn print_table_top() {
    println!("┏━━━━━━━━━━━━━━━━━━━━━━┳━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┳━━━━━━━━━━┳━━━━━━━━━━┳━━━━━━━━━━┳━━━━━━━━━━┓");
    println!("┃ {: ^20} ┃ {: ^50} ┃ {: ^8} ┃ {: ^8} ┃ {: ^8} ┃ {: ^8} ┃",
            "Domain", "Instance", "CPU(G)", "IO(GB)", "NET(GB)", "Disk(GB)");
    println!("┡━━━━━━━━━━━━━━━━━━━━━━╇━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━╇━━━━━━━━━━╇━━━━━━━━━━╇━━━━━━━━━━╇━━━━━━━━━━┩");
}

fn print_table_bottom() {
    println!("└──────────────────────┴────────────────────────────────────────────────────┴──────────┴──────────┴──────────┴──────────┘");
}

fn main() {

    const DIG : i64 = 1000000000;
    let mut stats = ("", "", 0, 0, 0, 0, 0, 0);

    // Get target node and user
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Target node is required.");
        return;
    }
    let host = format!("{}:22", args[1]);
    let user = env::var("USER").expect("$USER is not set");

    // Run 'virsh domstats' in target node
    let domstats = run_ssh( &user,
                            &host,
                            "sudo virsh domstats --cpu-total --interface --block \
                            | grep -e Domain: -e cpu.time -e bytes -e allocation -e capacity");
    print_table_top();

/* Sample output
Domain: 'vm-001'
  cpu.time=287227998002
  cpu.user=3420000000
  cpu.system=47170000000
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

    // Collect status from each domain(instance)
    for buff in domstats.lines() {
        let line = buff.trim();

        // Ask instance name if line contains domain name
        if line.contains("Domain: ") {
            // When next data set comes(Domain line), print previous data set
            if stats.0.len() > 0 {
                let cmd = format!("{} {} {}", "sudo virsh dumpxml", stats.0, 
                                                "| grep nova:name | sed -r 's/<nova:name>(.*)<\\/nova:name>/\\1/'");
                let result = run_ssh( &user, &host, &cmd);
                // Use "|".join(), with headers[]?
                println!("| {: <20} | {: <50} | {: >8} | {: >8} | {: >8} |{: >4} /{: >4}|", 
                            stats.0, result.trim(), stats.2/DIG, stats.3/DIG, stats.4/DIG, stats.5/DIG, stats.6/DIG);
                stats = ("", "", 0, 0, 0, 0, 0, 0);
            }
            let domain :Vec<&str> = line.split('\'').collect();
            stats.0 = domain[1];
            continue;
        }

        // Split A.B.C=D lines for each data
        let keyvalue: Vec<&str> = line.split('=').collect();
        if keyvalue.len() <= 0 {
            continue;
        }
        let key: Vec<&str> = keyvalue[0].split('.').collect();
        let value = keyvalue[1].parse::<i64>().unwrap();

        // Collect data for cpu, block, net
        match key[0] {
            "cpu" => {
                if *key.last().unwrap() == "time" {
                    stats.2 += value;
                }
            },
            "block" => {
                match *key.last().unwrap() {
                    "bytes" => stats.3 += value,
                    "allocation" => stats.5 += value,
                    "capacity" => stats.6 += value,
                    _ => (),
                }
            },
            "net" => {
                if *key.last().unwrap() == "bytes" {
                    stats.4 += value;
                }
            },
            _ => (),
        }
    }
    print_table_bottom();

}