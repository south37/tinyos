all: entry.o vectors.o syscall.o initcode entryother

entry.o: entry.S
	cc -fno-pic -gdwarf-2 -m64 -mcmodel=kernel -mno-red-zone -c entry.S

syscall.o: syscall.S
	cc -fno-pic -gdwarf-2 -m64 -mcmodel=kernel -mno-red-zone -c syscall.S

vectors.o: vectors.S
	cc -fno-pic -gdwarf-2 -m64 -mcmodel=kernel -mno-red-zone -c vectors.S

initcode: initcode.S
	cc -fno-pic -gdwarf-2 -m64 -mcmodel=kernel -mno-red-zone -c initcode.S -o initcode.o
	ld -N -e _user_start -Ttext 0 -o initcode.out initcode.o
	objcopy -S -O binary initcode.out initcode

entryother: entryother.S
	cc -fno-pic -nostdinc -I. -c entryother.S -o entryother.o
	ld -N -e start -Ttext 0x7000 -o entryother.out entryother.o
	objcopy -S -O binary entryother.out entryother

clean: 
	rm -f entry.o vectors.o initcode initcode.out initcode.o entryother entryother.out entryother.o