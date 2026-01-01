all: entry.o vectors.o

entry.o: entry.S
	cc -fno-pic -gdwarf-2 -m64 -mcmodel=kernel -mno-red-zone -c entry.S

vectors.o: vectors.S
	cc -fno-pic -gdwarf-2 -m64 -mcmodel=kernel -mno-red-zone -c vectors.S

clean: 
	rm -f entry.o vectors.o	