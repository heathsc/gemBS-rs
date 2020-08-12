.PHONY: all clean distclean

all: bin
	cargo build --release 
	cp target/release/gem_bs bin/gemBS

bin:
	mkdir bin

clean:
	rm -rf target

distclean: clean
	rm -rf bin
