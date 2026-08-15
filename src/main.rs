//! domlist
//! # domlist
//!
//! domlist collects stat information from virsh. Mainly for OpenStack admin.

/*
TODO: forllow this guide
https://github.com/rust-lang/style-team/blob/master/guide/guide.md
*/

use clap::Parser;
use ssh2::{CheckResult, KnownHostFileKind, Session};
use std::env;
use std::io::Read;
use std::net::TcpStream;
use std::path::Path;
use std::process::ExitCode;

#[macro_use]
extern crate prettytable;
use prettytable::format;
use prettytable::Table;
use prettytable::{color, Attr};

/// Specify host or run in local
#[derive(Parser, Debug)]
#[clap(
    name = env!("CARGO_PKG_NAME"),
    version = env!("CARGO_PKG_VERSION"),
    author = env!("CARGO_PKG_AUTHORS"),
    about = env!("CARGO_PKG_DESCRIPTION"),
)]
struct Args {
    /// Target host name in FQDN or IP address
    host: Option<String>,
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

/// Quote a string so it can be safely embedded as a single word in a shell command
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Verify the remote host key against ~/.ssh/known_hosts to guard against MITM.
/// Refuses to continue on a definite mismatch; only warns if the host is simply unknown,
/// since requiring a prior manual `ssh` login for every target would be impractical here.
fn verify_host_key(ssn: &Session, hostname: &str) -> Result<(), String> {
    let (key, _key_type) = ssn
        .host_key()
        .ok_or_else(|| "Failed to get host key from session".to_string())?;

    let home = env::var("HOME").map_err(|_| "$HOME is not set".to_string())?;
    let mut known_hosts = ssn
        .known_hosts()
        .map_err(|e| format!("Failed to init known_hosts: {}", e))?;
    known_hosts
        .read_file(
            Path::new(&format!("{}/.ssh/known_hosts", home)),
            KnownHostFileKind::OpenSSH,
        )
        .map_err(|e| format!("Failed to read ~/.ssh/known_hosts: {}", e))?;

    match known_hosts.check(hostname, key) {
        CheckResult::Match => Ok(()),
        CheckResult::NotFound => {
            eprintln!(
                "Warning: {} is not in ~/.ssh/known_hosts; its identity could not be verified.",
                hostname
            );
            Ok(())
        }
        CheckResult::Mismatch => Err(format!(
            "Host key for {} does NOT match ~/.ssh/known_hosts! Refusing to connect (possible MITM).",
            hostname
        )),
        CheckResult::Failure => Err(format!("Failed to check host key for {}", hostname)),
    }
}

/// Open one authenticated SSH session, to be reused for every command that needs to run.
fn connect(user: &str, hostname: &str, port: u16) -> Result<Session, String> {
    let tcp = TcpStream::connect((hostname, port))
        .map_err(|e| format!("Failed to connect to {}:{}: {}", hostname, port, e))?;
    let mut ssn = Session::new().map_err(|e| format!("Failed to create SSH session: {}", e))?;
    ssn.set_tcp_stream(tcp);
    ssn.handshake()
        .map_err(|e| format!("SSH handshake with {} failed: {}", hostname, e))?;
    verify_host_key(&ssn, hostname)?;
    ssn.userauth_agent(user)
        .map_err(|e| format!("SSH agent auth for {} failed: {}", user, e))?;
    if !ssn.authenticated() {
        return Err(format!("SSH authentication as {} was not accepted", user));
    }
    Ok(ssn)
}

/// Run a command over an already-connected session and return its stdout.
/// Fails with the remote command's stderr if it exits non-zero.
fn exec(ssn: &Session, cmd: &str) -> Result<String, String> {
    let mut channel = ssn
        .channel_session()
        .map_err(|e| format!("Failed to open channel: {}", e))?;
    channel
        .exec(cmd)
        .map_err(|e| format!("Failed to run command over SSH: {}", e))?;

    let mut stdout = String::new();
    channel
        .read_to_string(&mut stdout)
        .map_err(|e| format!("Failed to read command output: {}", e))?;
    let mut stderr = String::new();
    channel
        .stderr()
        .read_to_string(&mut stderr)
        .map_err(|e| format!("Failed to read command stderr: {}", e))?;
    channel
        .wait_close()
        .map_err(|e| format!("Failed to close channel: {}", e))?;

    let status = channel
        .exit_status()
        .map_err(|e| format!("Failed to get exit status: {}", e))?;
    if status != 0 {
        return Err(format!(
            "Remote command exited with status {}: {}\ncommand: {}",
            status,
            stderr.trim(),
            cmd
        ));
    }
    Ok(stdout)
}

fn run() -> Result<(), String> {
    const GIGA: i64 = 1000000000;
    const MEGA: i64 = 1000000;
    let mut vmstats_list: Vec<VMStats> = vec![];

    // Get target node, port, user
    let args = Args::parse();
    let host: String = args.host.clone().unwrap_or(String::from("127.0.0.1"));
    let port: u16 = 22;
    let user: String = env::var("USER").map_err(|_| "$USER is not set".to_string())?;
    //println!(r"Connecting... : {}@{}", user, host);

    let ssn = connect(&user, &host, port)?;

    // Run 'virsh domstats' in target node
    let cmd: String = format!(
        "{} {} {}",
        "sudo virsh domstats",
        "--cpu-total --balloon --interface --block",
        "| grep -e Domain: -e cpu.time -e balloon -e bytes -e allocation -e capacity"
    );
    let domstats: String = exec(&ssn, &cmd)?;
    let mut index = 0;
    let mut domain_list: String = "".to_string();

    // Collect status from each domain(instance)
    for buff in domstats.lines() {
        let line = buff.trim();

        // Extract domain name from virsh command result
        if line.contains("Domain: ") {
            let domain: Vec<&str> = line.split('\'').collect();
            let domain_name = match domain.get(1) {
                Some(name) => *name,
                None => {
                    eprintln!("Warning: could not parse domain name from line: {}", line);
                    continue;
                }
            };
            let vmstats = VMStats {
                domain: domain_name.to_string(),
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
            domain_list += format!(" {}", shell_quote(domain_name)).as_str();
            continue;
        }

        // Split A.B.C=xxxx
        let keyvalue: Vec<&str> = line.split('=').collect();
        if keyvalue.len() < 2 {
            continue;
        }
        let key: Vec<&str> = keyvalue[0].split('.').collect();
        let value = match keyvalue[1].parse::<i64>() {
            Ok(v) => v,
            Err(_) => continue,
        };

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

    // Get instance name from domain name.
    // Emit one "domain|instance" line per domain (instance may be empty when the
    // domain has no nova:name, e.g. a non-OpenStack guest) so results can be matched
    // back by domain name instead of assuming both commands return the same line count.
    let cmd = format!(
        "{} {} {} {} {} {}",
        "for DOMAIN in",
        domain_list,
        "; do",
        r#"NAME=$(sudo virsh dumpxml "${DOMAIN}" | grep nova:name | sed -r 's/<nova:name>(.*)<\/nova:name>/\1/');"#,
        r#"echo "${DOMAIN}|${NAME}";"#,
        "done;"
    );
    let instances = exec(&ssn, &cmd)?;
    for line in instances.lines() {
        if let Some((domain, instance)) = line.trim().split_once('|') {
            if let Some(vmstats) = vmstats_list.iter_mut().find(|v| v.domain == domain) {
                vmstats.instance = instance.to_string();
            }
        }
    }

    // Print table
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
    table.set_titles(
        row![bc => "Domain", "Instance", "CPU(G)", "MEM(G)", "I/O(G)","NET(G)", "Disk(G)"],
    );

    // Determine top resource consumers from raw values before rendering, so highlighting
    // can't be fooled by two different raw values truncating to the same displayed number.
    let cpu_top: i64 = vmstats_list.iter().map(|v| v.cpu).max().unwrap_or(0);
    let io_top: i64 = vmstats_list.iter().map(|v| v.io).max().unwrap_or(0);
    let net_top: i64 = vmstats_list.iter().map(|v| v.net).max().unwrap_or(0);

    // Adding table row
    for vmstats in &vmstats_list {
        let row = table.add_row(row![
            vmstats.domain,
            vmstats.instance,
            r->(vmstats.cpu/GIGA).to_string(),
            r->format!("{}/{}", (vmstats.mem_cur/MEGA).to_string(),(vmstats.mem_max/MEGA).to_string()),
            r->(vmstats.io/GIGA).to_string(),
            r->(vmstats.net/GIGA).to_string(),
            r->format!("{}/{}", (vmstats.allocation/GIGA).to_string(),(vmstats.capacity/GIGA).to_string()),
        ]);

        // Coloring red for top resource consumer (compared on raw values, not display text)
        if vmstats.cpu == cpu_top {
            row.get_mut_cell(2)
                .unwrap()
                .style(Attr::ForegroundColor(color::RED));
        }
        if vmstats.io == io_top {
            row.get_mut_cell(4)
                .unwrap()
                .style(Attr::ForegroundColor(color::RED));
        }
        if vmstats.net == net_top {
            row.get_mut_cell(5)
                .unwrap()
                .style(Attr::ForegroundColor(color::RED));
        }
    }
    table.printstd();

    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}
