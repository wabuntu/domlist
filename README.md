# domlist
Collects stat infomation from virsh. Mainly for OpenStack admin.

You can download single binary from [here](https://github.com/wabuntu/domlist/tree/main/binaries) for each operationg system.

Please have your ssh-agent ready before you run.

```
$ eval `ssh-agent`
$ ssh-add .ssh/id_rsa
```

## sample

```
$ ./domlist computenode01.example.com
+--------------------+-------------------------------------------+---------+--------+--------+--------+---------+
|       Domain       |                 Instance                  | CPU(G)  | MEM(G) | I/O(G) | NET(G) | Disk(G) |
+--------------------+-------------------------------------------+---------+--------+--------+--------+---------+
| vmcloud01-aaaaaaaa | aaaaaaaaaaaaaaaa                          |  282606 |  33/33 |   7234 |  26510 | 158/171 |
| vmcloud01-bbbbbbbb | bbbbbbbbbbbbbbbbbbbbbbbb                  |  128276 |  16/16 |   5323 |   4618 | 334/515 |
| vmcloud01-cccccccc | cccccccccccccccccccccccccccccccccccccccc  |     111 |  16/16 |     28 |      0 |   5/171 |
+--------------------+-------------------------------------------+---------+--------+--------+--------+---------+
```
