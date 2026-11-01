# Contributing to the Apache OpenDAL™ Go Binding

This document covers building and testing the Go binding from source. For user-facing usage, see the [README](README.md) and the [user guide](https://opendal.apache.org/docs/bindings/go).

## Run Tests

### Behavior Tests

```bash
cd tests/behavior_tests
# Test a specific backend
export OPENDAL_TEST=memory
# Run all tests
CGO_ENABLE=0 go test -v -run TestBehavior
# Run specific test
CGO_ENABLE=0 go test -v -run TestBehavior/Write
# Run synchronously
CGO_ENABLE=0 GOMAXPROCS=1 go test -v -run TestBehavior
```

### Benchmark

```bash
cd tests/behavior_tests
# Benchmark a specific backend
export OPENDAL_TEST=memory

go test -bench .
```

<details>
  <summary>
  A benchmark between OpenDAL Go binding and aws-sdk-go on Silo S3 compatible storage
  </summary>

```
goos: linux
goarch: amd64
pkg: opendal_test
cpu: AMD EPYC 7763 64-Core Processor                
BenchmarkWrite
BenchmarkWrite/4KiB/OpenDAL
BenchmarkWrite/4KiB/OpenDAL-4         	     405	   3430652 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/4KiB/OpenDAL-4         	     313	   3781825 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/4KiB/OpenDAL-4         	     346	   3354802 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/4KiB/OpenDAL-4         	     397	   3374467 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/4KiB/OpenDAL-4         	     364	   6171386 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/4KiB/OpenDAL-4         	     388	   3789795 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/4KiB/AWS_S3
BenchmarkWrite/4KiB/AWS_S3-4          	     384	   3716060 ns/op	   62929 B/op	     312 allocs/op
BenchmarkWrite/4KiB/AWS_S3-4          	     402	   3145797 ns/op	   62077 B/op	     312 allocs/op
BenchmarkWrite/4KiB/AWS_S3-4          	     372	   3050911 ns/op	   61902 B/op	     312 allocs/op
BenchmarkWrite/4KiB/AWS_S3-4          	     400	   3081028 ns/op	   61556 B/op	     312 allocs/op
BenchmarkWrite/4KiB/AWS_S3-4          	     342	   3111741 ns/op	   61970 B/op	     312 allocs/op
BenchmarkWrite/4KiB/AWS_S3-4          	     464	   2933844 ns/op	   61617 B/op	     312 allocs/op
BenchmarkWrite/256KiB/OpenDAL
BenchmarkWrite/256KiB/OpenDAL-4       	     228	   6253966 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/256KiB/OpenDAL-4       	     190	   5859882 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/256KiB/OpenDAL-4       	     216	   6008253 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/256KiB/OpenDAL-4       	     200	   5958440 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/256KiB/OpenDAL-4       	     193	   5658798 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/256KiB/OpenDAL-4       	     210	   6250594 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/256KiB/AWS_S3
BenchmarkWrite/256KiB/AWS_S3-4        	     208	   5665223 ns/op	   90323 B/op	     312 allocs/op
BenchmarkWrite/256KiB/AWS_S3-4        	     196	   5368602 ns/op	   88719 B/op	     312 allocs/op
BenchmarkWrite/256KiB/AWS_S3-4        	     218	   6108315 ns/op	   90784 B/op	     312 allocs/op
BenchmarkWrite/256KiB/AWS_S3-4        	     217	   5828966 ns/op	   90379 B/op	     312 allocs/op
BenchmarkWrite/256KiB/AWS_S3-4        	     210	   6447429 ns/op	   90981 B/op	     312 allocs/op
BenchmarkWrite/256KiB/AWS_S3-4        	     177	   6323915 ns/op	   91548 B/op	     313 allocs/op
BenchmarkWrite/4MiB/OpenDAL
BenchmarkWrite/4MiB/OpenDAL-4         	      62	  19049500 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/4MiB/OpenDAL-4         	      54	  19271521 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/4MiB/OpenDAL-4         	      69	  17849900 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/4MiB/OpenDAL-4         	      73	  18117693 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/4MiB/OpenDAL-4         	      74	  18249816 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/4MiB/OpenDAL-4         	      62	  19556730 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/4MiB/AWS_S3
BenchmarkWrite/4MiB/AWS_S3-4          	      43	  29514084 ns/op	   92211 B/op	     320 allocs/op
BenchmarkWrite/4MiB/AWS_S3-4          	      39	  29297735 ns/op	   92514 B/op	     320 allocs/op
BenchmarkWrite/4MiB/AWS_S3-4          	      42	  28956593 ns/op	   92282 B/op	     320 allocs/op
BenchmarkWrite/4MiB/AWS_S3-4          	      42	  29218821 ns/op	   92282 B/op	     320 allocs/op
BenchmarkWrite/4MiB/AWS_S3-4          	      40	  28988214 ns/op	   91391 B/op	     320 allocs/op
BenchmarkWrite/4MiB/AWS_S3-4          	      43	  28668319 ns/op	   91242 B/op	     320 allocs/op
BenchmarkWrite/16MiB/OpenDAL
BenchmarkWrite/16MiB/OpenDAL-4        	      21	  53528117 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/16MiB/OpenDAL-4        	      21	  55328986 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/16MiB/OpenDAL-4        	      21	  54221620 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/16MiB/OpenDAL-4        	      20	  54044030 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/16MiB/OpenDAL-4        	      21	  53692610 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/16MiB/OpenDAL-4        	      21	  53288370 ns/op	     384 B/op	      10 allocs/op
BenchmarkWrite/16MiB/AWS_S3
BenchmarkWrite/16MiB/AWS_S3-4         	      12	  99294840 ns/op	   99850 B/op	     322 allocs/op
BenchmarkWrite/16MiB/AWS_S3-4         	      12	  97405067 ns/op	   99850 B/op	     322 allocs/op
BenchmarkWrite/16MiB/AWS_S3-4         	      12	  97906212 ns/op	   99848 B/op	     322 allocs/op
BenchmarkWrite/16MiB/AWS_S3-4         	      12	  98766864 ns/op	   96378 B/op	     322 allocs/op
BenchmarkWrite/16MiB/AWS_S3-4         	      12	  97967605 ns/op	   99850 B/op	     322 allocs/op
BenchmarkWrite/16MiB/AWS_S3-4         	      12	  97842268 ns/op	   96376 B/op	     322 allocs/op
BenchmarkRead
BenchmarkRead/4KiB/OpenDAL
BenchmarkRead/4KiB/OpenDAL-4          	     972	   1217197 ns/op	    4760 B/op	      15 allocs/op
BenchmarkRead/4KiB/OpenDAL-4          	     985	   1226752 ns/op	    4760 B/op	      15 allocs/op
BenchmarkRead/4KiB/OpenDAL-4          	     982	   1216827 ns/op	    4760 B/op	      15 allocs/op
BenchmarkRead/4KiB/OpenDAL-4          	     987	   1227682 ns/op	    4760 B/op	      15 allocs/op
BenchmarkRead/4KiB/OpenDAL-4          	     987	   1214420 ns/op	    4760 B/op	      15 allocs/op
BenchmarkRead/4KiB/OpenDAL-4          	     987	   1215074 ns/op	    4760 B/op	      15 allocs/op
BenchmarkRead/4KiB/AWS_S3
BenchmarkRead/4KiB/AWS_S3-4           	    1594	    759864 ns/op	   34988 B/op	     344 allocs/op
BenchmarkRead/4KiB/AWS_S3-4           	    1605	    751628 ns/op	   34990 B/op	     344 allocs/op
BenchmarkRead/4KiB/AWS_S3-4           	    1620	    744649 ns/op	   34991 B/op	     344 allocs/op
BenchmarkRead/4KiB/AWS_S3-4           	    1623	    752885 ns/op	   34994 B/op	     344 allocs/op
BenchmarkRead/4KiB/AWS_S3-4           	    1629	    742307 ns/op	   34991 B/op	     344 allocs/op
BenchmarkRead/4KiB/AWS_S3-4           	    1622	    750455 ns/op	   34992 B/op	     344 allocs/op
BenchmarkRead/256KiB/OpenDAL
BenchmarkRead/256KiB/OpenDAL-4        	     741	   1612259 ns/op	  262812 B/op	      15 allocs/op
BenchmarkRead/256KiB/OpenDAL-4        	     727	   1633753 ns/op	  262812 B/op	      15 allocs/op
BenchmarkRead/256KiB/OpenDAL-4        	     751	   1619770 ns/op	  262812 B/op	      15 allocs/op
BenchmarkRead/256KiB/OpenDAL-4        	     756	   1619435 ns/op	  262812 B/op	      15 allocs/op
BenchmarkRead/256KiB/OpenDAL-4        	     750	   1629211 ns/op	  262812 B/op	      15 allocs/op
BenchmarkRead/256KiB/OpenDAL-4        	     757	   1611646 ns/op	  262812 B/op	      15 allocs/op
BenchmarkRead/256KiB/AWS_S3
BenchmarkRead/256KiB/AWS_S3-4         	     736	   1655133 ns/op	 1209847 B/op	     365 allocs/op
BenchmarkRead/256KiB/AWS_S3-4         	     735	   1673714 ns/op	 1209874 B/op	     365 allocs/op
BenchmarkRead/256KiB/AWS_S3-4         	     735	   1653903 ns/op	 1209852 B/op	     365 allocs/op
BenchmarkRead/256KiB/AWS_S3-4         	     728	   1643222 ns/op	 1209855 B/op	     365 allocs/op
BenchmarkRead/256KiB/AWS_S3-4         	     726	   1674202 ns/op	 1209842 B/op	     365 allocs/op
BenchmarkRead/256KiB/AWS_S3-4         	     717	   1656092 ns/op	 1209876 B/op	     365 allocs/op
BenchmarkRead/4MiB/OpenDAL
BenchmarkRead/4MiB/OpenDAL-4          	     250	   4933426 ns/op	 4194991 B/op	      15 allocs/op
BenchmarkRead/4MiB/OpenDAL-4          	     255	   4874283 ns/op	 4194991 B/op	      15 allocs/op
BenchmarkRead/4MiB/OpenDAL-4          	     252	   4769956 ns/op	 4194992 B/op	      16 allocs/op
BenchmarkRead/4MiB/OpenDAL-4          	     244	   4800674 ns/op	 4194992 B/op	      16 allocs/op
BenchmarkRead/4MiB/OpenDAL-4          	     242	   4826974 ns/op	 4194992 B/op	      15 allocs/op
BenchmarkRead/4MiB/OpenDAL-4          	     254	   4882666 ns/op	 4194996 B/op	      16 allocs/op
BenchmarkRead/4MiB/AWS_S3
BenchmarkRead/4MiB/AWS_S3-4           	     122	  10379507 ns/op	21133764 B/op	     388 allocs/op
BenchmarkRead/4MiB/AWS_S3-4           	     100	  10838399 ns/op	21133778 B/op	     389 allocs/op
BenchmarkRead/4MiB/AWS_S3-4           	      92	  11831700 ns/op	21133738 B/op	     388 allocs/op
BenchmarkRead/4MiB/AWS_S3-4           	     100	  10002235 ns/op	21133764 B/op	     388 allocs/op
BenchmarkRead/4MiB/AWS_S3-4           	     100	  10416467 ns/op	21133731 B/op	     388 allocs/op
BenchmarkRead/4MiB/AWS_S3-4           	     100	  10452328 ns/op	21133719 B/op	     388 allocs/op
BenchmarkRead/16MiB/OpenDAL
BenchmarkRead/16MiB/OpenDAL-4         	      81	  14654347 ns/op	16777903 B/op	      15 allocs/op
BenchmarkRead/16MiB/OpenDAL-4         	      78	  14977822 ns/op	16777908 B/op	      16 allocs/op
BenchmarkRead/16MiB/OpenDAL-4         	      81	  14833921 ns/op	16777906 B/op	      16 allocs/op
BenchmarkRead/16MiB/OpenDAL-4         	      76	  14663855 ns/op	16777906 B/op	      16 allocs/op
BenchmarkRead/16MiB/OpenDAL-4         	      75	  14545209 ns/op	16777906 B/op	      16 allocs/op
BenchmarkRead/16MiB/OpenDAL-4         	      80	  14585250 ns/op	16777905 B/op	      16 allocs/op
BenchmarkRead/16MiB/AWS_S3
BenchmarkRead/16MiB/AWS_S3-4          	      52	  24826707 ns/op	102660545 B/op	     398 allocs/op
BenchmarkRead/16MiB/AWS_S3-4          	      46	  24588321 ns/op	102660529 B/op	     397 allocs/op
BenchmarkRead/16MiB/AWS_S3-4          	      44	  26526367 ns/op	102660527 B/op	     397 allocs/op
BenchmarkRead/16MiB/AWS_S3-4          	      44	  26996660 ns/op	102660522 B/op	     397 allocs/op
BenchmarkRead/16MiB/AWS_S3-4          	      42	  26319939 ns/op	102660552 B/op	     398 allocs/op
BenchmarkRead/16MiB/AWS_S3-4          	      45	  25286454 ns/op	102660542 B/op	     398 allocs/op
PASS
ok  	opendal_test	147.053s
```

**Diff** with [benchstat](https://pkg.go.dev/golang.org/x/perf/cmd/benchstat)
```
benchstat aws.txt opendal.txt
goos: linux
goarch: amd64
pkg: opendal_test
cpu: AMD EPYC 7763 64-Core Processor                
             │    aws.txt    │             opendal.txt              │
             │    sec/op     │    sec/op      vs base               │
Write/4KiB      3.096m ± 20%    3.606m ± 71%  +16.47% (p=0.015 n=6)
Write/256KiB    5.969m ± 10%    5.983m ±  5%        ~ (p=1.000 n=6)
Write/4MiB      29.10m ±  1%    18.65m ±  5%  -35.92% (p=0.002 n=6)
Write/16MiB     97.94m ±  1%    53.87m ±  3%  -45.00% (p=0.002 n=6)
Read/4KiB       751.0µ ±  1%   1217.0µ ±  1%  +62.04% (p=0.002 n=6)
Read/256KiB     1.656m ±  1%    1.620m ±  1%   -2.18% (p=0.002 n=6)
Read/4MiB      10.434m ± 13%    4.851m ±  2%  -53.51% (p=0.002 n=6)
Read/16MiB      25.80m ±  5%    14.66m ±  2%  -43.19% (p=0.002 n=6)
geomean         8.050m          6.461m        -19.73%

             │    aws.txt    │             opendal.txt             │
             │     B/op      │     B/op      vs base               │
Write/4KiB      61936.0 ± 2%     384.0 ± 0%  -99.38% (p=0.002 n=6)
Write/256KiB    90581.5 ± 2%     384.0 ± 0%  -99.58% (p=0.002 n=6)
Write/4MiB      92246.5 ± 1%     384.0 ± 0%  -99.58% (p=0.002 n=6)
Write/16MiB     99849.0 ± 3%     384.0 ± 0%  -99.62% (p=0.002 n=6)
Read/4KiB      34.171Ki ± 0%   4.648Ki ± 0%  -86.40% (p=0.002 n=6)
Read/256KiB    1181.5Ki ± 0%   256.7Ki ± 0%  -78.28% (p=0.002 n=6)
Read/4MiB      20.155Mi ± 0%   4.001Mi ± 0%  -80.15% (p=0.002 n=6)
Read/16MiB      97.90Mi ± 0%   16.00Mi ± 0%  -83.66% (p=0.002 n=6)
geomean         500.3Ki        14.12Ki       -97.18%

             │   aws.txt   │            opendal.txt            │
             │  allocs/op  │ allocs/op   vs base               │
Write/4KiB     312.00 ± 0%   10.00 ± 0%  -96.79% (p=0.002 n=6)
Write/256KiB   312.00 ± 0%   10.00 ± 0%  -96.79% (p=0.002 n=6)
Write/4MiB     320.00 ± 0%   10.00 ± 0%  -96.88% (p=0.002 n=6)
Write/16MiB    322.00 ± 0%   10.00 ± 0%  -96.89% (p=0.002 n=6)
Read/4KiB      344.00 ± 0%   15.00 ± 0%  -95.64% (p=0.002 n=6)
Read/256KiB    365.00 ± 0%   15.00 ± 0%  -95.89% (p=0.002 n=6)
Read/4MiB      388.00 ± 0%   15.50 ± 3%  -96.01% (p=0.002 n=6)
Read/16MiB     397.50 ± 0%   16.00 ± 6%  -95.97% (p=0.002 n=6)
geomean         343.6        12.40       -96.39%
```
</details>

## Development

The guide is based on Linux and Windows. For other platforms, please adjust the commands accordingly.

To develop the Go binding, you need to have the following dependencies installed:

- zstd
- Rust toolchain
- Go
- (Required for Windows) libffi-8.dll in the root of the workspace directory

We use `go workspace` to manage and build the dependencies. To set up the workspace, run the following commands:

<details>
  <summary>
  For Linux, macOS and Windows (MSVC)
  </summary>

<br/>

```bash
mkdir opendal_workspace
cd opendal_workspace
git clone --depth 1 git@github.com:apache/opendal.git
git clone --depth 1 git@github.com:apache/opendal-go-services.git

go work init
go work use ./opendal/bindings/go
go work use ./opendal/bindings/go/tests/behavior_tests

cp opendal/bindings/go/Makefile .

cd -
```

To build and run tests, run the following commands:

```bash
cd opendal_workspace

# specify the backend to test
export OPENDAL_TEST=fs
export OPENDAL_FS_ROOT=/tmp/opendal

make tests

cd -
```

To run the benchmarks, you can use the following command:

```bash
make bench
```

</details>
