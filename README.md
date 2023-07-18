# domlist
Collects stat infomation from virsh. Mainly for OpenStack admin.

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
