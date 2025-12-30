all: entry.o

entry.o: entry.S
	cc -fno-pic -gdwarf-2 -m64 -mcmodel=kernel -mno-red-zone -c entry.S

clean: 
	rm -f entry.o	