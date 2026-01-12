all: entry.o vectors.o syscall.o initcode

entry.o: entry.S
	cc -fno-pic -gdwarf-2 -m64 -mcmodel=kernel -mno-red-zone -c entry.S

syscall.o: syscall.S
	cc -fno-pic -gdwarf-2 -m64 -mcmodel=kernel -mno-red-zone -c syscall.S

vectors.o: vectors.S
	cc -fno-pic -gdwarf-2 -m64 -mcmodel=kernel -mno-red-zone -c vectors.S

initcode: initcode.S
	cc -fno-pic -gdwarf-2 -m64 -mcmodel=kernel -mno-red-zone -c initcode.S -o initcode.o
	ld -N -e start -Ttext 0 -o initcode.out initcode.o
	objcopy -S -O binary initcode.out initcode

clean: 
	rm -f entry.o vectors.o initcode initcode.out initcode.o