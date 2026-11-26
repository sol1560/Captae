# 打进安装包的 Rust 依赖

由 `scripts/gen-licenses.py` 生成（不要手改）：`captae-desktop` 和 `captae-cli` 在 macOS（aarch64）、Windows（x86_64 MSVC）、
Linux（x86_64 GNU）上真正编进去的第三方 crate，合成一张表，「平台」一列写只在哪些平台上编进去（「全部」就是三个平台都有），
不含只在编译时、测试时用的。许可证是「A OR B」的，Captae 有 MIT 就按 MIT、没有就按 Apache-2.0 用（「按」那一列）；
按这两种用的，版权行列在表里、许可证原文在文末；其余许可证（BSD、ISC、Zlib、MPL-2.0、Unicode-3.0、CDLA-Permissive-2.0……）
把这个 crate 自带的许可证文件原样附在文末。所有 crate 都没有改过，源码在各自的仓库和 crates.io 上对应的版本里
（MPL-2.0 的几个也一样：没有改动，源码见表里的仓库地址）。

一共 562 个。许可证表达式的分布：

- MIT OR Apache-2.0：246
- MIT：133
- Apache-2.0 OR MIT：72
- MIT/Apache-2.0：20
- Unicode-3.0：18
- Zlib OR Apache-2.0 OR MIT：13
- BSD-3-Clause：7
- Unlicense OR MIT：5
- MPL-2.0：5
- Apache-2.0：4
- BSL-1.0：3
- Zlib：3
- MIT OR Apache-2.0 OR Zlib：3
- Apache-2.0/MIT：2
- Apache-2.0 OR ISC OR MIT：2
- Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT：2
- MIT OR Zlib OR Apache-2.0：2
- BSD-3-Clause OR Apache-2.0：2
- BSD-3-Clause OR MIT OR Apache-2.0：2
- ISC：2
- Unlicense/MIT：2
- BSD-2-Clause OR Apache-2.0 OR MIT：2
- 0BSD OR MIT OR Apache-2.0：1
- BSD-3-Clause AND MIT：1
- BSD-3-Clause/MIT：1
- Apache-2.0 AND MIT：1
- CC0-1.0 OR MIT-0 OR Apache-2.0：1
- Apache-2.0 / MIT：1
- MIT OR BSD-3-Clause：1
- MIT AND BSD-3-Clause：1
- Apache-2.0 AND ISC：1
- Apache-2.0 OR BSL-1.0：1
- (MIT OR Apache-2.0) AND Unicode-3.0：1
- CDLA-Permissive-2.0：1

| crate | 版本 | 平台 | 许可证 | 按 | 来源 | 版权 |
|---|---|---|---|---|---|---|
| adler2 | 2.0.1 | 全部 | 0BSD OR MIT OR Apache-2.0 | MIT | https://github.com/oyvindln/adler2 | Copyright (C) Jonas Schievink <jonasschievink@gmail.com> |
| aead | 0.5.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/traits | Copyright (c) 2019 The RustCrypto Project Developers；Copyright (c) 2019 MobileCoin, LLC |
| aes | 0.8.4 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-ciphers | Copyright (c) 2018 Artyom Pavlov |
| aes-gcm | 0.10.3 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/AEADs | Copyright (c) 2019 The RustCrypto Project Developers |
| aes-kw | 0.2.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/key-wraps/tree/aes-kw | Copyright (c) 2021-2022 RustCrypto Developers |
| aho-corasick | 1.1.5 | 全部 | Unlicense OR MIT | MIT | https://github.com/BurntSushi/aho-corasick | Copyright (c) 2015 Andrew Gallant |
| alloc-no-stdlib | 2.0.4 | 全部 | BSD-3-Clause | BSD-3-Clause | https://github.com/dropbox/rust-alloc-no-stdlib | Copyright (c) 2016 Dropbox, Inc. |
| alloc-stdlib | 0.2.4 | 全部 | BSD-3-Clause | BSD-3-Clause | https://github.com/dropbox/rust-alloc-no-stdlib | 作者：Daniel Reiter Horn |
| anyhow | 1.0.104 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/anyhow | 作者：David Tolnay |
| argon2 | 0.5.3 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/password-hashes/tree/master/argon2 | Copyright (c) 2021-2024 The RustCrypto Project Developers |
| async-broadcast | 0.7.2 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/smol-rs/async-broadcast | Copyright 2020 Yoshua Wuyts；Copyright (c) 2020 Yoshua Wuyts |
| async-channel | 2.5.0 | Linux | Apache-2.0 OR MIT | MIT | https://github.com/smol-rs/async-channel | 作者：Stjepan Glavina |
| async-executor | 1.14.0 | Linux | Apache-2.0 OR MIT | MIT | https://github.com/smol-rs/async-executor | 作者：Stjepan Glavina、John Nunley |
| async-io | 2.6.0 | Linux | Apache-2.0 OR MIT | MIT | https://github.com/smol-rs/async-io | 作者：Stjepan Glavina |
| async-lock | 3.4.2 | Linux | Apache-2.0 OR MIT | MIT | https://github.com/smol-rs/async-lock | 作者：Stjepan Glavina |
| async-process | 2.5.0 | Linux | Apache-2.0 OR MIT | MIT | https://github.com/smol-rs/async-process | 作者：Stjepan Glavina |
| async-recursion | 1.1.1 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/dcchut/async-recursion | 作者：Robert Usher |
| async-signal | 0.2.14 | Linux | Apache-2.0 OR MIT | MIT | https://github.com/smol-rs/async-signal | 作者：John Nunley |
| async-task | 4.7.1 | Linux | Apache-2.0 OR MIT | MIT | https://github.com/smol-rs/async-task | 作者：Stjepan Glavina |
| async-trait | 0.1.92 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/async-trait | 作者：David Tolnay |
| atk | 0.18.2 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk3-rs | 作者：The gtk-rs Project Developers |
| atk-sys | 0.18.2 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk3-rs | 作者：The gtk-rs Project Developers |
| atomic-waker | 1.1.2 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/smol-rs/atomic-waker | Copyright (c) 2016 Alex Crichton；Copyright (c) 2017 The Tokio Authors |
| axum | 0.8.9 | 全部 | MIT | MIT | https://github.com/tokio-rs/axum | Copyright (c) 2019 axum Contributors |
| axum-core | 0.5.6 | 全部 | MIT | MIT | https://github.com/tokio-rs/axum | Copyright (c) 2019–2025 axum Contributors |
| base16ct | 0.2.0 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats/tree/master/base16ct | Copyright (c) 2014 Steve "Sc00bz" Thomas (steve at tobtu dot com)；Copyright (c) 2022 The RustCrypto Project Developers |
| base64 | 0.21.7 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/marshallpierce/rust-base64 | Copyright (c) 2015 Alice Maz |
| base64 | 0.22.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/marshallpierce/rust-base64 | Copyright (c) 2015 Alice Maz |
| base64 | 0.23.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/marshallpierce/rust-base64 | Copyright (c) 2025 Alice Maz, Marshall Pierce |
| base64ct | 1.8.3 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats | Copyright (c) 2014 Steve "Sc00bz" Thomas (steve at tobtu dot com)；Copyright (c) 2021-2025 The RustCrypto Project Developers |
| bit-set | 0.8.0 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/contain-rs/bit-set | Copyright (c) 2023 The Rust Project Developers |
| bit-vec | 0.8.0 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/contain-rs/bit-vec | Copyright (c) 2023 The Rust Project Developers |
| bitfields | 1.0.3 | 全部 | MIT | MIT | https://github.com/gregorygaines/bitfields-rs | 作者：Gregory Gaines |
| bitfields-impl | 1.0.3 | 全部 | MIT | MIT | https://github.com/gregorygaines/bitfields-rs | 作者：Gregory Gaines |
| bitflags | 1.3.2 | 全部 | MIT/Apache-2.0 | MIT | https://github.com/bitflags/bitflags | Copyright (c) 2014 The Rust Project Developers |
| bitflags | 2.13.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/bitflags/bitflags | Copyright (c) 2014 The Rust Project Developers |
| bitvec | 1.1.1 | 全部 | MIT | MIT | https://github.com/bitvecto-rs/bitvec | Copyright (c) 2018 myrrlyn (Alexander Payne) |
| blake2 | 0.10.6 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/hashes | Copyright (c) 2015-2016 The blake2-rfc Developers, Cesar Barros；Copyright (c) 2017 Artyom Pavlov |
| block-buffer | 0.10.4 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/utils | Copyright (c) 2018-2019 The RustCrypto Project Developers |
| block-padding | 0.3.3 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/utils | Copyright (c) 2018-2019 The RustCrypto Project Developers |
| block2 | 0.6.2 | macOS | MIT | MIT | https://github.com/madsmtm/objc2 | 作者：Mads Marquart |
| blocking | 1.7.0 | Linux | Apache-2.0 OR MIT | MIT | https://github.com/smol-rs/blocking | — |
| blowfish | 0.9.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-ciphers | Copyright (c) 2006-2009 Graydon Hoare；Copyright (c) 2009-2013 Mozilla Foundation |
| brotli | 8.0.4 | 全部 | BSD-3-Clause AND MIT | BSD-3-Clause + MIT | https://github.com/dropbox/rust-brotli | Copyright (c) 2016 Dropbox, Inc.；Copyright (c) 2009, 2010, 2013-2016 by the Brotli Authors. |
| brotli-decompressor | 5.0.3 | 全部 | BSD-3-Clause/MIT | MIT | https://github.com/dropbox/rust-brotli-decompressor | Copyright (c) 2016 Dropbox, Inc. |
| buffer-redux | 1.1.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dignifiedquire/buffer-redux | Copyright 2016-2018 Austin Bonander and The Rust Project Developers；Copyright (c) 2016-2022 Austin Bonander and The Rust Project Developers |
| bumpalo | 3.20.3 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/fitzgen/bumpalo | Copyright (c) 2019 Nick Fitzgerald |
| bytemuck | 1.25.2 | 全部 | Zlib OR Apache-2.0 OR MIT | MIT | https://github.com/Lokathor/bytemuck | Copyright (c) 2019 Daniel "Lokathor" Gee. |
| byteorder | 1.5.0 | 全部 | Unlicense OR MIT | MIT | https://github.com/BurntSushi/byteorder | Copyright (c) 2015 Andrew Gallant |
| byteorder-lite | 0.1.0 | 全部 | Unlicense OR MIT | MIT | https://github.com/image-rs/byteorder-lite | Copyright (c) 2015 Andrew Gallant |
| bytes | 1.12.1 | 全部 | MIT | MIT | https://github.com/tokio-rs/bytes | Copyright (c) 2018 Carl Lerche |
| cairo-rs | 0.18.5 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk-rs-core | 作者：The gtk-rs Project Developers |
| cairo-sys-rs | 0.18.2 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk-rs-core | 作者：The gtk-rs Project Developers |
| camellia | 0.1.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-ciphers | Copyright (c) 2021 Shun Sakai |
| camino | 1.2.6 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/camino-rs/camino | 作者：Without Boats、Ashley Williams、Steve Klabnik、Rain |
| captae-remote | 0.2.0 | 全部 | MIT | MIT | https://crates.io/crates/captae-remote | — |
| cargo-platform | 0.1.9 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/cargo | — |
| cargo_metadata | 0.19.2 | 全部 | MIT | MIT | https://github.com/oli-obk/cargo_metadata | 作者：Oliver Schneider |
| cast5 | 0.11.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-ciphers | Copyright (c) 2018-2019 Friedel Ziegelmayer；Copyright (c) 2018-2019 The RustCrypto Project Developers |
| cbc | 0.1.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-modes | Copyright (c) 2018-2022 RustCrypto Developers；Copyright (c) 2018 Artyom Pavlov |
| cfb | 0.7.3 | 全部 | MIT | MIT | https://github.com/mdsteele/rust-cfb | Copyright (c) 2017 Matthew D. Steele |
| cfb-mode | 0.8.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-modes | Copyright (c) 2018-2022 RustCrypto Developers；Copyright (c) 2018 Artyom Pavlov |
| cfg-if | 1.0.5 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/cfg-if | Copyright (c) 2014 Alex Crichton |
| chrono | 0.4.45 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/chronotope/chrono | Copyright (c) 2014, Kang Seonghoon. |
| cipher | 0.4.4 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/traits | Copyright (c) 2016-2020 RustCrypto Developers |
| clipboard-rs | 0.3.5 | 全部 | MIT | MIT | https://github.com/ChurchTao/clipboard-rs | Copyright (c) 2024 桃树夭 |
| clipboard-win | 5.4.1 | Windows | BSL-1.0 | BSL-1.0 | https://github.com/DoumanAsh/clipboard-win | 作者：Douman |
| cmac | 0.7.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/MACs | Copyright (c) 2017 Artyom Pavlov |
| concurrent-queue | 2.5.0 | Linux | Apache-2.0 OR MIT | MIT | https://github.com/smol-rs/concurrent-queue | 作者：Stjepan Glavina、Taiki Endo、John Nunley |
| const-oid | 0.9.6 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats/tree/master/const-oid | Copyright (c) 2020-2022 The RustCrypto Project Developers |
| convert_case | 0.10.0 | 全部 | MIT | MIT | https://github.com/rutrum/convert-case | Copyright (c) 2025 rutrum |
| cookie | 0.18.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/SergioBenitez/cookie-rs | Copyright 2017 Sergio Benitez；Copyright 2014 Alex Chricton；Copyright (c) 2017 Sergio Benitez；Copyright (c) 2014 Alex Crichton |
| core-foundation | 0.10.1 | macOS | MIT OR Apache-2.0 | MIT | https://github.com/servo/core-foundation-rs | Copyright (c) 2012-2013 Mozilla Foundation |
| core-foundation | 0.9.4 | macOS | MIT OR Apache-2.0 | MIT | https://github.com/servo/core-foundation-rs | Copyright (c) 2012-2013 Mozilla Foundation |
| core-foundation-sys | 0.8.7 | macOS | MIT OR Apache-2.0 | MIT | https://github.com/servo/core-foundation-rs | Copyright (c) 2012-2013 Mozilla Foundation |
| core-graphics | 0.25.0 | macOS | MIT OR Apache-2.0 | MIT | https://github.com/servo/core-foundation-rs | Copyright (c) 2012-2013 Mozilla Foundation |
| core-graphics-types | 0.2.0 | macOS | MIT OR Apache-2.0 | MIT | https://github.com/servo/core-foundation-rs | Copyright (c) 2012-2013 Mozilla Foundation |
| cpufeatures | 0.2.17 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/utils | Copyright (c) 2020-2025 The RustCrypto Project Developers |
| crc24 | 0.1.6 | 全部 | MIT/Apache-2.0 | MIT | https://github.com/sellibitze/crc24-rs.git | 作者：Sebastian Gesemann |
| crc32fast | 1.5.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/srijs/rust-crc32fast | Copyright (c) 2018 Sam Rijs, Alex Crichton and contributors |
| crossbeam-channel | 0.5.17 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/crossbeam-rs/crossbeam | Copyright (c) 2019 The Crossbeam Project Developers；copyright protection under copyright law or other applicable laws.；Copyright (c) 2009 The Go Authors. All rights reserved. |
| crossbeam-utils | 0.8.23 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/crossbeam-rs/crossbeam | Copyright (c) 2019 The Crossbeam Project Developers |
| crypto-bigint | 0.5.5 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/crypto-bigint | Copyright (c) 2021 The RustCrypto Project Developers |
| crypto-common | 0.1.7 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/traits | Copyright (c) 2021 RustCrypto Developers |
| cssparser | 0.36.0 | 全部 | MPL-2.0 | MPL-2.0 | https://github.com/servo/rust-cssparser | 作者：Simon Sapin |
| cssparser-macros | 0.6.1 | 全部 | MPL-2.0 | MPL-2.0 | https://github.com/servo/rust-cssparser | 作者：Simon Sapin |
| ctor | 0.8.0 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/mmastrac/rust-ctor | 作者：Matt Mastracci |
| ctor-proc-macro | 0.0.7 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/mmastrac/rust-ctor | 作者：Matt Mastracci |
| ctr | 0.9.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-modes | Copyright (c) 2018-2022 RustCrypto Developers；Copyright (c) 2018 Artyom Pavlov |
| curve25519-dalek | 4.1.3 | 全部 | BSD-3-Clause | BSD-3-Clause | https://github.com/dalek-cryptography/curve25519-dalek/tree/main/curve25519-dalek | Copyright (c) 2016-2021 isis agora lovecruft. All rights reserved.；Copyright (c) 2016-2021 Henry de Valence. All rights reserved.；Copyright (c) 2012 The Go Authors. All rights reserved. |
| curve25519-dalek-derive | 0.1.1 | Windows + Linux | MIT/Apache-2.0 | MIT | https://github.com/dalek-cryptography/curve25519-dalek | — |
| cx448 | 0.1.1 | 全部 | BSD-3-Clause | BSD-3-Clause | https://github.com/dignifiedquire/cx448 | — |
| darling | 0.20.11 | 全部 | MIT | MIT | https://github.com/TedDriggs/darling | Copyright (c) 2017 Ted Driggs |
| darling | 0.24.1 | 全部 | MIT | MIT | https://github.com/TedDriggs/darling | Copyright (c) 2017 Ted Driggs |
| darling_core | 0.20.11 | 全部 | MIT | MIT | https://github.com/TedDriggs/darling | Copyright (c) 2017 Ted Driggs |
| darling_core | 0.24.1 | 全部 | MIT | MIT | https://github.com/TedDriggs/darling | Copyright (c) 2017 Ted Driggs |
| darling_macro | 0.20.11 | 全部 | MIT | MIT | https://github.com/TedDriggs/darling | Copyright (c) 2017 Ted Driggs |
| darling_macro | 0.24.1 | 全部 | MIT | MIT | https://github.com/TedDriggs/darling | Copyright (c) 2017 Ted Driggs |
| data-encoding | 2.11.1 | 全部 | MIT | MIT | https://github.com/ia0/data-encoding | Copyright (c) 2015-2020 Julien Cretin；Copyright (c) 2017-2020 Google Inc. |
| dbl | 0.3.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/utils | Copyright (c) 2018-2019 The RustCrypto Project Developers |
| dbus | 0.9.12 | Linux | Apache-2.0/MIT | MIT | https://github.com/diwic/dbus-rs | Copyright 2014-2018 David Henningsson <diwic@ubuntu.com> and other contributors；Copyright (c) 2014-2018 David Henningsson <diwic@ubuntu.com> and other contributors |
| der | 0.7.10 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats/tree/master/der | Copyright (c) 2020-2023 The RustCrypto Project Developers |
| deranged | 0.5.8 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/jhpratt/deranged | Copyright 2024 Jacob Pratt et al.；Copyright (c) 2024 Jacob Pratt et al. |
| derive_builder | 0.20.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/colin-kiegel/rust-derive-builder | Copyright (c) 2016 rust-derive-builder contributors |
| derive_builder_core | 0.20.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/colin-kiegel/rust-derive-builder | Copyright (c) 2016 rust-derive-builder contributors |
| derive_builder_macro | 0.20.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/colin-kiegel/rust-derive-builder | Copyright (c) 2016 rust-derive-builder contributors |
| derive_more | 2.1.1 | 全部 | MIT | MIT | https://github.com/JelteF/derive_more | Copyright (c) 2016 Jelte Fennema |
| derive_more-impl | 2.1.1 | 全部 | MIT | MIT | https://github.com/JelteF/derive_more | Copyright (c) 2016 Jelte Fennema |
| des | 0.8.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-ciphers | Copyright 2017 Gulshan Singh；Copyright (c) 2017 Gulshan Singh, Antoni Boucher, Artyom Pavlov |
| digest | 0.10.7 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/traits | Copyright (c) 2017 Artyom Pavlov |
| directories | 6.0.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/soc/directories-rs | Copyright (c) 2018 directories-rs contributors |
| dirs | 6.0.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/soc/dirs-rs | Copyright (c) 2018-2019 dirs-rs contributors |
| dirs-sys | 0.5.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dirs-dev/dirs-sys-rs | Copyright (c) 2018-2019 dirs-rs contributors |
| dispatch2 | 0.3.1 | macOS | Zlib OR Apache-2.0 OR MIT | MIT | https://github.com/madsmtm/objc2 | 作者：Mads Marquart、Mary |
| displaydoc | 0.2.7 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/yaahc/displaydoc | 作者：Jane Lusby |
| dlopen2 | 0.8.2 | Linux | MIT | MIT | https://github.com/OpenByteDev/dlopen2 | 作者：Szymon Wieloch、Ahmed Masud、OpenByte |
| dlopen2_derive | 0.4.3 | Linux | MIT | MIT | https://github.com/OpenByteDev/dlopen2 | 作者：Szymon Wieloch、OpenByte |
| dom_query | 0.27.0 | 全部 | MIT | MIT | https://github.com/niklak/dom_query | Copyright (c) 2023 Mykola Humanov |
| dpi | 0.1.2 | 全部 | Apache-2.0 AND MIT | Apache-2.0 + MIT | https://github.com/rust-windowing/winit | Copyright (c) 2018 Jorge Aparicio；copyright:；Copyright © 2005-2020 Rich Felker, et al.；Copyright © 1993,2004 Sun Microsystems or；Copyright © 2003-2011 David Schultz or；Copyright © 2003-2009 Steven G. Kargl or；Copyright © 2003-2009 Bruce D. Evans or；Copyright © 2008 Stephen L. Moshier or；Copyright © 2017-2018 Arm Limited |
| dsa | 0.6.3 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/signatures/tree/master/dsa | Copyright 2018-2022 RustCrypto Developers；Copyright (c) 2018-2023 RustCrypto Developers |
| dtoa | 1.0.11 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/dtoa | 作者：David Tolnay |
| dtoa-short | 0.3.5 | 全部 | MPL-2.0 | MPL-2.0 | https://github.com/upsuper/dtoa-short | 作者：Xidorn Quan |
| dunce | 1.0.5 | 全部 | CC0-1.0 OR MIT-0 OR Apache-2.0 | Apache-2.0 | https://gitlab.com/kornelski/dunce | 作者：Kornel |
| dyn-clone | 1.0.20 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/dyn-clone | 作者：David Tolnay |
| eax | 0.5.0 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/AEADs | Copyright (c) 2020 The RustCrypto Project Developers |
| ecdsa | 0.16.9 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/signatures/tree/master/ecdsa | Copyright 2018-2022 RustCrypto Developers；Copyright (c) 2018-2022 RustCrypto Developers |
| ed25519 | 2.2.3 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/signatures/tree/master/ed25519 | Copyright 2018-2022 RustCrypto Developers；Copyright (c) 2018-2023 RustCrypto Developers |
| ed25519-dalek | 2.2.0 | 全部 | BSD-3-Clause | BSD-3-Clause | https://github.com/dalek-cryptography/curve25519-dalek/tree/main/ed25519-dalek | Copyright (c) 2017-2019 isis agora lovecruft. All rights reserved. |
| elliptic-curve | 0.13.8 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/traits/tree/master/elliptic-curve | Copyright (c) 2020-2022 RustCrypto Developers |
| embed_plist | 1.2.2 | macOS | MIT OR Apache-2.0 | MIT | https://github.com/nvzqz/embed-plist-rs | Copyright (c) 2020 Nikolai Vazquez |
| endi | 1.1.1 | Linux | MIT | MIT | https://github.com/zeenix/endi | 作者：Zeeshan Ali Khan |
| enumflags2 | 0.7.12 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/meithecatte/enumflags2 | Copyright 2017-2023 Maik Klein, Maja Kądziołka；Copyright (c) 2017-2023 Maik Klein, Maja Kądziołka |
| enumflags2_derive | 0.7.12 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/meithecatte/enumflags2 | Copyright [2017] [Maik Klein]；Copyright (c) 2017 Maik Klein |
| equivalent | 1.0.2 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/indexmap-rs/equivalent | Copyright (c) 2016--2023 |
| erased-serde | 0.4.10 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/erased-serde | 作者：David Tolnay |
| errno | 0.3.14 | macOS + Linux | MIT OR Apache-2.0 | MIT | https://github.com/lambda-fairy/rust-errno | Copyright (c) 2014 Chris Wong |
| error-code | 3.4.0 | Windows | BSL-1.0 | BSL-1.0 | https://github.com/DoumanAsh/error-code | 作者：Douman |
| event-listener | 5.4.2 | Linux | Apache-2.0 OR MIT | MIT | https://github.com/smol-rs/event-listener | 作者：Stjepan Glavina、John Nunley |
| event-listener-strategy | 0.5.4 | Linux | Apache-2.0 OR MIT | MIT | https://github.com/smol-rs/event-listener-strategy | 作者：John Nunley |
| fallible-iterator | 0.3.0 | 全部 | MIT/Apache-2.0 | MIT | https://github.com/sfackler/rust-fallible-iterator | Copyright (c) 2015 The rust-openssl-verify Developers |
| fallible-streaming-iterator | 0.1.9 | 全部 | MIT/Apache-2.0 | MIT | https://github.com/sfackler/fallible-streaming-iterator | Copyright (c) 2016 The fallible-streaming-iterator Developers |
| fastrand | 2.5.0 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/smol-rs/fastrand | 作者：Stjepan Glavina |
| fax | 0.2.7 | macOS | MIT | MIT | https://github.com/pdf-rs/fax | Copyright © 2021 The pdf-rs contributers. |
| fdeflate | 0.3.7 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/image-rs/fdeflate | 作者：The image-rs Developers |
| ff | 0.13.1 | 全部 | MIT/Apache-2.0 | MIT | https://github.com/zkcrypto/ff | Copyright (c) 2017 Sean Bowe |
| field-offset | 0.3.6 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/Diggsey/rust-field-offset | Copyright (c) 2016-2021 Diggory Blake, and other contributors. |
| flate2 | 1.1.10 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/flate2-rs | Copyright (c) 2014-2026 Alex Crichton |
| fnv | 1.0.7 | 全部 | Apache-2.0 / MIT | MIT | https://github.com/servo/rust-fnv | Copyright (c) 2017 Contributors |
| foldhash | 0.1.5 | 全部 | Zlib | Zlib | https://github.com/orlp/foldhash | Copyright (c) 2024 Orson Peters |
| foldhash | 0.2.0 | 全部 | Zlib | Zlib | https://github.com/orlp/foldhash | Copyright (c) 2024 Orson Peters |
| foreign-types | 0.5.0 | macOS | MIT/Apache-2.0 | MIT | https://github.com/sfackler/foreign-types | Copyright (c) 2017 The foreign-types Developers |
| foreign-types-macros | 0.2.4 | macOS | MIT/Apache-2.0 | MIT | https://github.com/sfackler/foreign-types | Copyright (c) 2017 The foreign-types Developers |
| foreign-types-shared | 0.3.1 | macOS | MIT/Apache-2.0 | MIT | https://github.com/sfackler/foreign-types | Copyright (c) 2017 The foreign-types Developers |
| form_urlencoded | 1.2.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/servo/rust-url | Copyright (c) 2013-2016 The rust-url developers |
| funty | 2.0.0 | 全部 | MIT | MIT | https://github.com/myrrlyn/funty | Copyright (c) 2020 myrrlyn (Alexander Payne) |
| futures-channel | 0.3.34 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/futures-rs | Copyright (c) 2016 Alex Crichton；Copyright (c) 2017 The Tokio Authors |
| futures-core | 0.3.34 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/futures-rs | Copyright (c) 2016 Alex Crichton；Copyright (c) 2017 The Tokio Authors |
| futures-executor | 0.3.34 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/futures-rs | Copyright (c) 2016 Alex Crichton；Copyright (c) 2017 The Tokio Authors |
| futures-io | 0.3.34 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/futures-rs | Copyright (c) 2016 Alex Crichton；Copyright (c) 2017 The Tokio Authors |
| futures-lite | 2.6.1 | Linux | Apache-2.0 OR MIT | MIT | https://github.com/smol-rs/futures-lite | Copyright (c) 2016 Alex Crichton；Copyright (c) 2017 The Tokio Authors |
| futures-macro | 0.3.34 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/futures-rs | Copyright (c) 2016 Alex Crichton；Copyright (c) 2017 The Tokio Authors |
| futures-sink | 0.3.34 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/futures-rs | Copyright (c) 2016 Alex Crichton；Copyright (c) 2017 The Tokio Authors |
| futures-task | 0.3.34 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/futures-rs | Copyright (c) 2016 Alex Crichton；Copyright (c) 2017 The Tokio Authors |
| futures-util | 0.3.34 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/futures-rs | Copyright (c) 2016 Alex Crichton；Copyright (c) 2017 The Tokio Authors |
| gdk | 0.18.2 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk3-rs | 作者：The gtk-rs Project Developers |
| gdk-pixbuf | 0.18.5 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk-rs-core | 作者：The gtk-rs Project Developers |
| gdk-pixbuf-sys | 0.18.0 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk-rs-core | 作者：The gtk-rs Project Developers |
| gdk-sys | 0.18.2 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk3-rs | 作者：The gtk-rs Project Developers |
| gdkwayland-sys | 0.18.2 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk3-rs | 作者：The gtk-rs Project Developers |
| gdkx11 | 0.18.2 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk3-rs | 作者：The gtk-rs Project Developers |
| gdkx11-sys | 0.18.2 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk3-rs | 作者：The gtk-rs Project Developers |
| generic-array | 0.14.7 | 全部 | MIT | MIT | https://github.com/fizyk20/generic-array.git | Copyright (c) 2015 Bartłomiej Kamiński |
| gethostname | 1.1.0 | Linux | Apache-2.0 | Apache-2.0 | https://codeberg.org/swsnr/gethostname.rs.git | 作者：Sebastian Wiesner |
| getrandom | 0.2.17 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-random/getrandom | Copyright (c) 2018-2024 The rust-random Project Developers；Copyright (c) 2014 The Rust Project Developers |
| getrandom | 0.3.4 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-random/getrandom | Copyright (c) 2018-2025 The rust-random Project Developers；Copyright (c) 2014 The Rust Project Developers |
| getrandom | 0.4.3 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-random/getrandom | Copyright (c) 2018-2026 The rust-random Project Developers；Copyright (c) 2014 The Rust Project Developers |
| ghash | 0.5.1 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/universal-hashes | Copyright (c) 2019 RustCrypto Developers |
| gio | 0.18.4 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk-rs-core | 作者：The gtk-rs Project Developers |
| gio-sys | 0.18.1 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk-rs-core | 作者：The gtk-rs Project Developers |
| glib | 0.18.5 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk-rs-core | 作者：The gtk-rs Project Developers |
| glib-macros | 0.18.5 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk-rs-core | 作者：The gtk-rs Project Developers |
| glib-sys | 0.18.1 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk-rs-core | 作者：The gtk-rs Project Developers |
| glob | 0.3.4 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/glob | Copyright (c) 2014 The Rust Project Developers |
| gobject-sys | 0.18.0 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk-rs-core | 作者：The gtk-rs Project Developers |
| group | 0.13.0 | 全部 | MIT/Apache-2.0 | MIT | https://github.com/zkcrypto/group | 作者：Sean Bowe、Jack Grigg |
| gtk | 0.18.2 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk3-rs | 作者：The gtk-rs Project Developers |
| gtk-sys | 0.18.2 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk3-rs | 作者：The gtk-rs Project Developers |
| gtk3-macros | 0.18.2 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk3-rs | 作者：The gtk-rs Project Developers |
| half | 2.7.1 | macOS | MIT OR Apache-2.0 | MIT | https://github.com/VoidStarKat/half-rs | 作者：Kathryn Long |
| hashbrown | 0.12.3 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/hashbrown | Copyright (c) 2016 Amanieu d'Antras |
| hashbrown | 0.15.5 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/hashbrown | Copyright (c) 2016 Amanieu d'Antras |
| hashbrown | 0.17.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/hashbrown | Copyright (c) 2016 Amanieu d'Antras |
| hashlink | 0.10.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/kyren/hashlink | 作者：kyren |
| heck | 0.4.1 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/withoutboats/heck | Copyright (c) 2015 The Rust Project Developers |
| heck | 0.5.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/withoutboats/heck | Copyright (c) 2015 The Rust Project Developers |
| hex | 0.4.3 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/KokaKiwi/rust-hex | Copyright (c) 2013-2014 The Rust Project Developers.；Copyright (c) 2015-2020 The rust-hex Developers |
| hkdf | 0.12.4 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/KDFs/ | Copyright (c) 2015-2018 Vlad Filippov；Copyright (c) 2018-2021 RustCrypto Developers |
| hmac | 0.12.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/MACs | Copyright (c) 2017 Artyom Pavlov |
| html5ever | 0.38.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/servo/html5ever | Copyright (c) 2014 The html5ever Project Developers |
| http | 1.5.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/hyperium/http | Copyright 2017 http-rs authors；Copyright (c) 2017 http-rs authors |
| http-body | 1.1.0 | 全部 | MIT | MIT | https://github.com/hyperium/http-body | Copyright (c) 2019-2026 Sean McArthur & Hyper Contributors |
| http-body-util | 0.1.5 | 全部 | MIT | MIT | https://github.com/hyperium/http-body | Copyright (c) 2019-2026 Sean McArthur & Hyper Contributors |
| http-range | 0.1.5 | 全部 | MIT | MIT | https://github.com/bancek/rust-http-range.git | Copyright (c) 2016 Luka Zakrajšek |
| httparse | 1.10.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/seanmonstar/httparse | Copyright (c) 2015-2025 Sean McArthur |
| httpdate | 1.0.3 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/pyfisch/httpdate | Copyright (c) 2016 Pyfisch |
| hyper | 1.11.1 | 全部 | MIT | MIT | https://github.com/hyperium/hyper | Copyright (c) 2014-2026 Sean McArthur |
| hyper-rustls | 0.27.10 | 全部 | Apache-2.0 OR ISC OR MIT | MIT | https://github.com/rustls/hyper-rustls | Copyright (c) 2016, Joseph Birr-Pixton <jpixton@gmail.com>；Copyright (c) 2016 Joseph Birr-Pixton <jpixton@gmail.com> |
| hyper-util | 0.1.21 | 全部 | MIT | MIT | https://github.com/hyperium/hyper-util | Copyright (c) 2023-2025 Sean McArthur |
| iana-time-zone | 0.1.65 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/strawlab/iana-time-zone | Copyright 2020 Andrew Straw；Copyright (c) 2020 Andrew D. Straw |
| ico | 0.5.0 | 全部 | MIT | MIT | https://github.com/mdsteele/rust-ico | Copyright (c) 2018 Matthew D. Steele |
| icu_collections | 2.3.0 | 全部 | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| icu_locale_core | 2.3.0 | 全部 | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| icu_normalizer | 2.3.0 | 全部 | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| icu_normalizer_data | 2.3.0 | 全部 | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| icu_properties | 2.3.0 | 全部 | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| icu_properties_data | 2.3.0 | 全部 | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| icu_provider | 2.3.1 | 全部 | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| idea | 0.5.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-ciphers | Copyright (c) 2017 Damian Czaja |
| ident_case | 1.0.1 | 全部 | MIT/Apache-2.0 | MIT | https://github.com/TedDriggs/ident_case | 作者：Ted Driggs |
| idna | 1.1.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/servo/rust-url/ | Copyright (c) 2013-2025 The rust-url developers |
| idna_adapter | 1.2.2 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/hsivonen/idna_adapter | Copyright (c) The rust-url developers |
| if-addrs | 0.15.0 | 全部 | MIT OR BSD-3-Clause | MIT | https://github.com/messense/if-addrs | Copyright 2018 MaidSafe.net limited.；Copyright 2020 messense |
| image | 0.25.10 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/image-rs/image | 作者：The image-rs Developers |
| indexmap | 1.9.3 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/bluss/indexmap | Copyright (c) 2016--2017 |
| indexmap | 2.14.2 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/indexmap-rs/indexmap | Copyright (c) 2016--2017 |
| infer | 0.19.0 | 全部 | MIT | MIT | https://github.com/bojand/infer | Copyright (c) 2019 Bojan |
| inout | 0.1.4 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/utils | Copyright (c) 2022 The RustCrypto Project Developers；Copyright (c) 2022 Artyom Pavlov |
| ipnet | 2.12.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/krisprice/ipnet | Copyright 2017 Juniper Networks, Inc. |
| is-docker | 0.2.0 | Linux | MIT | MIT | https://github.com/TheLarkInn/is-docker | Copyright (c) 2023 Sean Larkin |
| is-wsl | 0.4.0 | Linux | MIT | MIT | https://github.com/TheLarkInn/is-wsl | Copyright (c) 2023 Sean Larkin |
| itoa | 1.0.18 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/itoa | 作者：David Tolnay |
| javascriptcore-rs | 1.1.2 | Linux | MIT | MIT | https://github.com/tauri-apps/javascriptcore-rs | Copyright (c) 2013-2021, The Gtk-rs Project Developers.；Copyright (c) 2021, Tauri Programme within The Commons Conservancy. |
| javascriptcore-rs-sys | 1.1.1 | Linux | MIT | MIT | https://github.com/tauri-apps/javascriptcore-rs | Copyright (c) 2013-2017, The Gtk-rs Project Developers. |
| json-patch | 3.0.1 | 全部 | MIT/Apache-2.0 | MIT | https://github.com/idubrov/json-patch | Copyright (c) 2017 Ivan Dubrov |
| jsonptr | 0.6.3 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/chanced/jsonptr | Copyright 2024 Chance Dinkins；Copyright (c) 2022 Chance Dinkins |
| k256 | 0.13.4 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/elliptic-curves/tree/master/k256 | Copyright (c) 2020-2024 RustCrypto Developers |
| keccak | 0.1.6 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/sponges/tree/master/keccak | Copyright (c) 2018-2022 RustCrypto Developers |
| keyboard-types | 0.7.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/pyfisch/keyboard-types | Copyright (c) 2017 Pyfisch |
| lazy_static | 1.5.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang-nursery/lazy-static.rs | Copyright (c) 2010 The Rust Project Developers |
| libc | 0.2.189 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/libc | Copyright (c) The Rust Project Developers |
| libdbus-sys | 0.2.7 | Linux | Apache-2.0/MIT | MIT | https://github.com/diwic/dbus-rs | Copyright 2014-2018 David Henningsson <diwic@ubuntu.com> and other contributors；Copyright (c) 2014-2018 David Henningsson <diwic@ubuntu.com> and other contributors |
| libm | 0.2.16 | 全部 | MIT | MIT | https://github.com/rust-lang/compiler-builtins | Copyright (c) 2018 Jorge Aparicio；copyright:；Copyright © 2005-2020 Rich Felker, et al.；Copyright © 1993,2004 Sun Microsystems or；Copyright © 2003-2011 David Schultz or；Copyright © 2003-2009 Steven G. Kargl or；Copyright © 2003-2009 Bruce D. Evans or；Copyright © 2008 Stephen L. Moshier or；Copyright © 2017-2018 Arm Limited |
| libsqlite3-sys | 0.35.0 | 全部 | MIT | MIT | https://github.com/rusqlite/rusqlite | Copyright (c) 2014 The rusqlite developers |
| linux-raw-sys | 0.12.1 | Linux | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | MIT | https://github.com/sunfishcode/linux-raw-sys | 作者：Dan Gohman |
| litemap | 0.8.3 | 全部 | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| lock_api | 0.4.14 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/Amanieu/parking_lot | Copyright (c) 2016 The Rust Project Developers |
| log | 0.4.34 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/log | Copyright (c) 2014 The Rust Project Developers |
| markup5ever | 0.38.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/servo/html5ever | Copyright (c) 2014 The html5ever Project Developers |
| matchers | 0.2.0 | 全部 | MIT | MIT | https://github.com/hawkw/matchers | Copyright (c) 2019 Eliza Weisman |
| matchit | 0.8.4 | 全部 | MIT AND BSD-3-Clause | MIT + BSD-3-Clause | https://github.com/ibraheemdev/matchit | Copyright (c) 2022 Ibraheem Ahmed；Copyright (c) 2013, Julien Schmidt |
| md-5 | 0.10.6 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/hashes | Copyright (c) 2006-2009 Graydon Hoare；Copyright (c) 2009-2013 Mozilla Foundation；Copyright (c) 2016 Artyom Pavlov |
| memchr | 2.8.3 | 全部 | Unlicense OR MIT | MIT | https://github.com/BurntSushi/memchr | Copyright (c) 2015 Andrew Gallant |
| memoffset | 0.9.1 | Linux | MIT | MIT | https://github.com/Gilnaa/memoffset | Copyright (c) 2017 Gilad Naaman |
| mime | 0.3.17 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/hyperium/mime | Copyright (c) 2014 Sean McArthur |
| miniz_oxide | 0.8.9 | 全部 | MIT OR Zlib OR Apache-2.0 | MIT | https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide | Copyright 2013-2014 RAD Game Tools and Valve Software；Copyright 2010-2014 Rich Geldreich and Tenacious Software LLC；Copyright (c) 2017 Frommi；Copyright (c) 2017-2024 oyvindln；Copyright (c) 2020 Frommi |
| miniz_oxide | 0.9.1 | 全部 | MIT OR Zlib OR Apache-2.0 | MIT | https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide | Copyright 2013-2014 RAD Game Tools and Valve Software；Copyright 2010-2014 Rich Geldreich and Tenacious Software LLC；Copyright (c) 2017 Frommi；Copyright (c) 2017-2024 oyvindln；Copyright (c) 2020 Frommi |
| mio | 1.2.3 | 全部 | MIT | MIT | https://github.com/tokio-rs/mio | Copyright (c) 2014 Carl Lerche and other MIO contributors |
| moxcms | 0.8.1 | 全部 | BSD-3-Clause OR Apache-2.0 | Apache-2.0 | https://github.com/awxkee/moxcms.git | Copyright 2024 Radzivon Bartoshyk；Copyright (c) Radzivon Bartoshyk. All rights reserved. |
| muda | 0.19.3 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/tauri-apps/muda | Copyright (c) 2022-2022 Tauri Programme within The Commons Conservancy |
| new_debug_unreachable | 1.0.6 | 全部 | MIT | MIT | https://github.com/mbrubeck/rust-debug-unreachable | Copyright (c) 2015 Jonathan Reem |
| nom | 8.0.0 | 全部 | MIT | MIT | https://github.com/rust-bakery/nom | Copyright (c) 2014-2019 Geoffroy Couprie |
| nu-ansi-term | 0.50.3 | 全部 | MIT | MIT | https://github.com/nushell/nu-ansi-term | Copyright (c) 2014 Benjamin Sago；Copyright (c) 2021-2022 The Nushell Project Developers |
| num-bigint-dig | 0.8.6 | 全部 | MIT/Apache-2.0 | MIT | https://github.com/dignifiedquire/num-bigint | Copyright (c) 2014 The Rust Project Developers |
| num-conv | 0.2.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/jhpratt/num-conv | Copyright (c) Jacob Pratt |
| num-integer | 0.1.47 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-num/num-integer | Copyright (c) 2014 The Rust Project Developers |
| num-iter | 0.1.46 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-num/num-iter | Copyright (c) 2014 The Rust Project Developers |
| num-traits | 0.2.19 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-num/num-traits | Copyright (c) 2014 The Rust Project Developers |
| num_enum | 0.7.6 | 全部 | BSD-3-Clause OR MIT OR Apache-2.0 | MIT | https://github.com/illicitonion/num_enum | Copyright (c) 2018, Daniel Wagner-Hall |
| num_enum_derive | 0.7.6 | 全部 | BSD-3-Clause OR MIT OR Apache-2.0 | MIT | https://github.com/illicitonion/num_enum | Copyright (c) 2018, Daniel Wagner-Hall |
| num_threads | 0.1.7 | macOS + Linux | MIT OR Apache-2.0 | MIT | https://github.com/jhpratt/num_threads | Copyright 2021 Jacob Pratt；Copyright (c) 2021 Jacob Pratt |
| objc2 | 0.6.4 | macOS | MIT | MIT | https://github.com/madsmtm/objc2 | 作者：Mads Marquart |
| objc2-app-kit | 0.3.2 | macOS | Zlib OR Apache-2.0 OR MIT | MIT | https://github.com/madsmtm/objc2 | — |
| objc2-cloud-kit | 0.3.2 | macOS | Zlib OR Apache-2.0 OR MIT | MIT | https://github.com/madsmtm/objc2 | — |
| objc2-core-data | 0.3.2 | macOS | Zlib OR Apache-2.0 OR MIT | MIT | https://github.com/madsmtm/objc2 | — |
| objc2-core-foundation | 0.3.2 | macOS | Zlib OR Apache-2.0 OR MIT | MIT | https://github.com/madsmtm/objc2 | — |
| objc2-core-graphics | 0.3.2 | macOS | Zlib OR Apache-2.0 OR MIT | MIT | https://github.com/madsmtm/objc2 | — |
| objc2-core-image | 0.3.2 | macOS | Zlib OR Apache-2.0 OR MIT | MIT | https://github.com/madsmtm/objc2 | — |
| objc2-core-text | 0.3.2 | macOS | Zlib OR Apache-2.0 OR MIT | MIT | https://github.com/madsmtm/objc2 | — |
| objc2-core-video | 0.3.2 | macOS | Zlib OR Apache-2.0 OR MIT | MIT | https://github.com/madsmtm/objc2 | — |
| objc2-encode | 4.1.0 | macOS | MIT | MIT | https://github.com/madsmtm/objc2 | 作者：Mads Marquart |
| objc2-exception-helper | 0.1.1 | macOS | Zlib OR Apache-2.0 OR MIT | MIT | https://github.com/madsmtm/objc2 | 作者：Mads Marquart |
| objc2-foundation | 0.3.2 | macOS | MIT | MIT | https://github.com/madsmtm/objc2 | — |
| objc2-quartz-core | 0.3.2 | macOS | Zlib OR Apache-2.0 OR MIT | MIT | https://github.com/madsmtm/objc2 | — |
| objc2-web-kit | 0.3.2 | macOS | Zlib OR Apache-2.0 OR MIT | MIT | https://github.com/madsmtm/objc2 | — |
| ocb3 | 0.1.0 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/AEADs | Copyright (c) 2023 The RustCrypto Project Developers |
| once_cell | 1.21.4 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/matklad/once_cell | 作者：Aleksey Kladov |
| opaque-debug | 0.3.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/utils | Copyright (c) 2018-2024 The RustCrypto Project Developers |
| open | 5.4.4 | 全部 | MIT | MIT | https://github.com/Byron/open-rs | Copyright © `2015` `Sebastian Thiel` |
| option-ext | 0.2.0 | 全部 | MPL-2.0 | MPL-2.0 | https://github.com/soc/option-ext.git | 作者：Simon Ochsenreither |
| ordered-stream | 0.2.0 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/danieldg/ordered-stream | 作者：Daniel De Graaf、Zeeshan Ali Khan |
| p256 | 0.13.2 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/elliptic-curves/tree/master/p256 | Copyright (c) 2020-2023 RustCrypto Developers |
| p384 | 0.13.1 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/elliptic-curves/tree/master/p384 | Copyright (c) 2020-2021 RustCrypto Developers |
| p521 | 0.13.3 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/elliptic-curves/tree/master/p521 | Copyright (c) 2020-2022 RustCrypto Developers |
| pango | 0.18.3 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk-rs-core | 作者：The gtk-rs Project Developers |
| pango-sys | 0.18.0 | Linux | MIT | MIT | https://github.com/gtk-rs/gtk-rs-core | 作者：The gtk-rs Project Developers |
| parking | 2.2.1 | Linux | Apache-2.0 OR MIT | MIT | https://github.com/smol-rs/parking | Copyright 2014-2020 The Rust Project Developers |
| parking_lot | 0.12.5 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/Amanieu/parking_lot | Copyright (c) 2016 The Rust Project Developers |
| parking_lot_core | 0.9.12 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/Amanieu/parking_lot | Copyright (c) 2016 The Rust Project Developers |
| password-hash | 0.5.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/traits/tree/master/password-hash | Copyright (c) 2020-2023 RustCrypto Developers |
| pem-rfc7468 | 0.7.0 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats/tree/master/pem-rfc7468 | Copyright (c) 2021 The RustCrypto Project Developers |
| percent-encoding | 2.3.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/servo/rust-url/ | Copyright (c) 2013-2025 The rust-url developers |
| pgp | 0.21.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rpgp/rpgp | 作者：dignifiedquire |
| phf | 0.13.1 | 全部 | MIT | MIT | https://github.com/rust-phf/rust-phf | Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi |
| phf_generator | 0.13.1 | 全部 | MIT | MIT | https://github.com/rust-phf/rust-phf | Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi |
| phf_macros | 0.13.1 | 全部 | MIT | MIT | https://github.com/rust-phf/rust-phf | Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi |
| phf_shared | 0.13.1 | 全部 | MIT | MIT | https://github.com/rust-phf/rust-phf | Copyright (c) 2014-2022 Steven Fackler, Yuki Okushi |
| pin-project-lite | 0.2.17 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/taiki-e/pin-project-lite | — |
| piper | 0.2.5 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/smol-rs/piper | 作者：Stjepan Glavina、John Nunley |
| pkcs1 | 0.7.5 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats/tree/master/pkcs1 | Copyright (c) 2021-2023 The RustCrypto Project Developers |
| pkcs8 | 0.10.2 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats/tree/master/pkcs8 | Copyright (c) 2020-2023 The RustCrypto Project Developers |
| plist | 1.10.1 | 全部 | MIT | MIT | https://github.com/ebarnard/rust-plist/ | Copyright (c) 2015 Edward Barnard |
| png | 0.17.16 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/image-rs/image-png | Copyright (c) 2015 nwin |
| png | 0.18.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/image-rs/image-png | Copyright (c) 2015 nwin |
| polling | 3.11.0 | Linux | Apache-2.0 OR MIT | MIT | https://github.com/smol-rs/polling | 作者：Stjepan Glavina、John Nunley |
| polyval | 0.6.2 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/universal-hashes | Copyright (c) 2019-2023 RustCrypto Developers |
| potential_utf | 0.1.6 | 全部 | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| powerfmt | 0.2.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/jhpratt/powerfmt | Copyright 2023 Jacob Pratt et al.；Copyright (c) 2023 Jacob Pratt et al. |
| ppv-lite86 | 0.2.21 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/cryptocorrosion/cryptocorrosion | Copyright 2019 The CryptoCorrosion Contributors；Copyright (c) 2019 The CryptoCorrosion Contributors |
| precomputed-hash | 0.1.1 | 全部 | MIT | MIT | https://github.com/emilio/precomputed-hash | Copyright (c) 2017 Emilio Cobos Álvarez |
| primeorder | 0.13.6 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/elliptic-curves/tree/master/primeorder | Copyright (c) 2020-2023 RustCrypto Developers |
| proc-macro-crate | 1.3.1 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/bkchr/proc-macro-crate | 作者：Bastian Köcher |
| proc-macro-crate | 2.0.2 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/bkchr/proc-macro-crate | 作者：Bastian Köcher |
| proc-macro-crate | 3.5.0 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/bkchr/proc-macro-crate | 作者：Bastian Köcher |
| proc-macro-error | 1.0.4 | Linux | MIT OR Apache-2.0 | MIT | https://gitlab.com/CreepySkeleton/proc-macro-error | Copyright 2019-2020 CreepySkeleton <creepy-skeleton@yandex.ru>；Copyright (c) 2019-2020 CreepySkeleton |
| proc-macro-error-attr | 1.0.4 | Linux | MIT OR Apache-2.0 | MIT | https://gitlab.com/CreepySkeleton/proc-macro-error | Copyright 2019-2020 CreepySkeleton <creepy-skeleton@yandex.ru>；Copyright (c) 2019-2020 CreepySkeleton |
| proc-macro2 | 1.0.107 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/proc-macro2 | 作者：David Tolnay、Alex Crichton |
| pxfm | 0.1.30 | 全部 | BSD-3-Clause OR Apache-2.0 | Apache-2.0 | https://github.com/awxkee/pxfm | Copyright 2024 Radzivon Bartoshyk；Copyright (c) Radzivon Bartoshyk. All rights reserved. |
| quick-error | 2.0.1 | macOS | MIT/Apache-2.0 | MIT | http://github.com/tailhook/quick-error | Copyright (c) 2015 The quick-error Developers |
| quick-xml | 0.42.0 | 全部 | MIT | MIT | https://github.com/tafia/quick-xml | Copyright (c) 2016 Johann Tuffe |
| quote | 1.0.47 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/quote | 作者：David Tolnay |
| radium | 0.7.0 | 全部 | MIT | MIT | https://github.com/bitvecto-rs/radium | Copyright (c) 2019 kneecaw (Nika Layzell) |
| rand | 0.8.8 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-random/rand | Copyright 2018 Developers of the Rand project；Copyright (c) 2014 The Rust Project Developers |
| rand | 0.9.5 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-random/rand | Copyright 2018 Developers of the Rand project；Copyright (c) 2014 The Rust Project Developers |
| rand_chacha | 0.3.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-random/rand | Copyright 2018 Developers of the Rand project；Copyright (c) 2014 The Rust Project Developers |
| rand_chacha | 0.9.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-random/rand | Copyright 2018 Developers of the Rand project；Copyright (c) 2014 The Rust Project Developers |
| rand_core | 0.6.4 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-random/rand | Copyright 2018 Developers of the Rand project；Copyright (c) 2014 The Rust Project Developers |
| rand_core | 0.9.5 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-random/rand | Copyright 2018 Developers of the Rand project；Copyright (c) 2014 The Rust Project Developers |
| raw-window-handle | 0.6.2 | 全部 | MIT OR Apache-2.0 OR Zlib | MIT | https://github.com/rust-windowing/raw-window-handle | Copyright (c) 2019 Osspial；Copyright (c) 2020 Osspial |
| rcgen | 0.14.10 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rustls/rcgen | Copyright (c) 2019-2026 est31 <MTest31@outlook.com> and contributors |
| regex | 1.13.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/regex | Copyright (c) 2014 The Rust Project Developers |
| regex-automata | 0.4.18 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/regex | Copyright (c) 2014 The Rust Project Developers |
| regex-syntax | 0.8.11 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/regex | Copyright (c) 2014 The Rust Project Developers |
| replace_with | 0.1.8 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/alecmocatta/replace_with | 作者：Alec Mocatta |
| reqwest | 0.12.28 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/seanmonstar/reqwest | Copyright 2016 Sean McArthur；Copyright (c) 2016-2025 Sean McArthur |
| rfc6979 | 0.4.0 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/signatures/tree/master/rfc6979 | Copyright 2018-2022 RustCrypto Developers；Copyright (c) 2018-2022 RustCrypto Developers |
| rfd | 0.16.0 | 全部 | MIT | MIT | https://github.com/PolyMeilex/rfd | Copyright (c) 2022 Bartłomiej Maryńczak |
| ring | 0.17.14 | 全部 | Apache-2.0 AND ISC | Apache-2.0 + ISC | https://github.com/briansmith/ring | Copyright (c) 2009 The Go Authors. All rights reserved.；Copyright 2015 The Chromium Authors. All rights reserved.；Copyright 2015-2025 Brian Smith. |
| ripemd | 0.1.3 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/hashes | Copyright (c) 2021 RustCrypto Developers |
| rsa | 0.9.10 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/RSA | 作者：RustCrypto Developers、dignifiedquire |
| rusqlite | 0.37.0 | 全部 | MIT | MIT | https://github.com/rusqlite/rusqlite | Copyright (c) 2014 The rusqlite developers |
| rustc-hash | 2.1.3 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/rust-lang/rustc-hash | 作者：The Rust Project Developers |
| rustix | 1.1.5 | Linux | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | MIT | https://github.com/bytecodealliance/rustix | 作者：Dan Gohman、Jakub Konka |
| rustls | 0.23.45 | 全部 | Apache-2.0 OR ISC OR MIT | MIT | https://github.com/rustls/rustls | Copyright (c) 2016, Joseph Birr-Pixton <jpixton@gmail.com>；Copyright (c) 2016 Joseph Birr-Pixton <jpixton@gmail.com> |
| rustls-pki-types | 1.15.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rustls/pki-types | Copyright 2023 Dirkjan Ochtman；Copyright (c) 2023 Dirkjan Ochtman <dirkjan@ochtman.nl> |
| rustls-webpki | 0.103.15 | 全部 | ISC | ISC | https://github.com/rustls/webpki | Copyright 2015 Brian Smith. |
| rustversion | 1.0.23 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/rustversion | 作者：David Tolnay |
| ryu | 1.0.23 | 全部 | Apache-2.0 OR BSL-1.0 | Apache-2.0 | https://github.com/dtolnay/ryu | 作者：David Tolnay |
| same-file | 1.0.6 | 全部 | Unlicense/MIT | MIT | https://github.com/BurntSushi/same-file | Copyright (c) 2017 Andrew Gallant |
| schemars | 0.8.22 | 全部 | MIT | MIT | https://github.com/GREsau/schemars | Copyright (c) 2019 Graham Esau |
| schemars_derive | 0.8.22 | 全部 | MIT | MIT | https://github.com/GREsau/schemars | Copyright (c) 2019 Graham Esau |
| scopeguard | 1.2.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/bluss/scopeguard | Copyright (c) 2016-2019 Ulrik Sverdrup "bluss" and scopeguard developers |
| sec1 | 0.7.3 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats/tree/master/sec1 | Copyright (c) 2021-2022 The RustCrypto Project Developers |
| selectors | 0.36.1 | 全部 | MPL-2.0 | MPL-2.0 | https://github.com/servo/stylo | 作者：The Servo Project Developers |
| semver | 1.0.28 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/semver | 作者：David Tolnay |
| serde | 1.0.229 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/serde-rs/serde | 作者：Erick Tryzelaar、David Tolnay |
| serde-untagged | 0.1.9 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/serde-untagged | 作者：David Tolnay |
| serde_core | 1.0.229 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/serde-rs/serde | 作者：Erick Tryzelaar、David Tolnay |
| serde_derive | 1.0.229 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/serde-rs/serde | 作者：Erick Tryzelaar、David Tolnay |
| serde_derive_internals | 0.29.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/serde-rs/serde | 作者：Erick Tryzelaar、David Tolnay |
| serde_json | 1.0.151 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/serde-rs/json | 作者：Erick Tryzelaar、David Tolnay |
| serde_path_to_error | 0.1.20 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/path-to-error | 作者：David Tolnay |
| serde_repr | 0.1.21 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/serde-repr | 作者：David Tolnay |
| serde_spanned | 0.6.9 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/toml-rs/toml | Copyright (c) Individual contributors |
| serde_spanned | 1.1.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/toml-rs/toml | Copyright (c) Individual contributors |
| serde_urlencoded | 0.7.1 | 全部 | MIT/Apache-2.0 | MIT | https://github.com/nox/serde_urlencoded | Copyright (c) 2016 Anthony Ramine |
| serde_with | 3.23.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/jonasbb/serde_with/ | Copyright (c) 2015 |
| serde_with_macros | 3.23.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/jonasbb/serde_with/ | Copyright (c) 2015 |
| serdect | 0.2.0 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats/tree/master/serdect | Copyright (c) 2022 The RustCrypto Project Developers |
| serdect | 0.3.0 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats | Copyright (c) 2022 The RustCrypto Project Developers |
| serialize-to-javascript | 0.1.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/chippers/serialize-to-javascript | Copyright (c) 2021 Chip Reed |
| serialize-to-javascript-impl | 0.1.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/chippers/serialize-to-javascript | Copyright (c) 2021 Chip Reed |
| servo_arc | 0.4.3 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/servo/stylo | 作者：The Servo Project Developers |
| sha1 | 0.10.7 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/hashes | Copyright (c) 2006-2009 Graydon Hoare；Copyright (c) 2009-2013 Mozilla Foundation；Copyright (c) 2016 Artyom Pavlov |
| sha1-checked | 0.10.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/hashes | Copyright (c) 2024 The RustCrypto Project Developers |
| sha2 | 0.10.9 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/hashes | Copyright (c) 2006-2009 Graydon Hoare；Copyright (c) 2009-2013 Mozilla Foundation；Copyright (c) 2016 Artyom Pavlov |
| sha3 | 0.10.9 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/hashes | Copyright (c) 2006-2009 Graydon Hoare；Copyright (c) 2009-2013 Mozilla Foundation；Copyright (c) 2014 Sébastien Martini；Copyright (c) 2016-2023 Artyom Pavlov, Marek Kotewicz |
| sharded-slab | 0.1.7 | 全部 | MIT | MIT | https://github.com/hawkw/sharded-slab | Copyright (c) 2019 Eliza Weisman |
| signal-hook-registry | 1.4.8 | macOS + Linux | MIT OR Apache-2.0 | MIT | https://github.com/vorner/signal-hook | Copyright (c) 2017 tokio-jsonrpc developers |
| signature | 2.2.0 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/traits/tree/master/signature | Copyright (c) 2018-2023 RustCrypto Developers |
| simd-adler32 | 0.3.10 | 全部 | MIT | MIT | https://github.com/mcountryman/simd-adler32 | Copyright (c) [2021] [Marvin Countryman] |
| siphasher | 1.0.4 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/jedisct1/rust-siphash | Copyright 2012-2016 The Rust Project Developers.；Copyright 2016-2026 Frank Denis. |
| slab | 0.4.12 | 全部 | MIT | MIT | https://github.com/tokio-rs/slab | Copyright (c) 2019 Carl Lerche |
| smallvec | 1.16.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/servo/rust-smallvec | Copyright (c) 2018 The Servo Project Developers |
| snafu | 0.9.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/shepmaster/snafu | Copyright 2019- Jake Goulding；Copyright (c) 2019- Jake Goulding |
| snafu-derive | 0.9.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/shepmaster/snafu | Copyright 2019- Jake Goulding；Copyright (c) 2019- Jake Goulding |
| socket2 | 0.6.5 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/socket2 | Copyright (c) 2014 Alex Crichton |
| softbuffer | 0.4.8 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/rust-windowing/softbuffer | Copyright 2022 Kirill Chibisov |
| soup3 | 0.5.0 | Linux | MIT | MIT | https://gitlab.gnome.org/World/Rust/soup3-rs | Copyright (c) 2013-2017, The Gtk-rs Project Developers. |
| soup3-sys | 0.5.0 | Linux | MIT | MIT | https://gitlab.gnome.org/World/Rust/soup3-rs | Copyright (c) 2013-2017, The Gtk-rs Project Developers. |
| spake2 | 0.4.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/PAKEs/tree/master/spake2 | Copyright (c) 2017-2023 Brian Warner |
| spin | 0.9.9 | 全部 | MIT | MIT | https://github.com/mvdnes/spin-rs.git | Copyright (c) 2014 Mathijs van de Nes |
| spki | 0.7.3 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats/tree/master/spki | Copyright (c) 2021-2023 The RustCrypto Project Developers |
| stable_deref_trait | 1.2.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/storyyeller/stable_deref_trait | Copyright (c) 2017 Robert Grosse |
| string_cache | 0.9.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/servo/string-cache | Copyright (c) 2012-2013 Mozilla Foundation |
| strsim | 0.11.1 | 全部 | MIT | MIT | https://github.com/rapidfuzz/strsim-rs | Copyright (c) 2015 Danny Guo；Copyright (c) 2016 Titus Wormer <tituswormer@gmail.com>；Copyright (c) 2018 Akash Kurdekar |
| subtle | 2.6.1 | 全部 | BSD-3-Clause | BSD-3-Clause | https://github.com/dalek-cryptography/subtle | Copyright (c) 2016-2017 Isis Agora Lovecruft, Henry de Valence. All rights reserved.；Copyright (c) 2016-2024 Isis Agora Lovecruft. All rights reserved. |
| swift-rs | 1.0.8 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/Brendonovich/swift-rs | Copyright 2023 The swift-rs developers；Copyright (c) 2023 The swift-rs Developers |
| syn | 1.0.109 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/syn | 作者：David Tolnay |
| syn | 2.0.119 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/syn | 作者：David Tolnay |
| syn | 3.0.6 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/syn | 作者：David Tolnay |
| sync_wrapper | 1.0.2 | 全部 | Apache-2.0 | Apache-2.0 | https://github.com/Actyx/sync_wrapper | 作者：Actyx AG |
| synstructure | 0.14.0 | 全部 | MIT | MIT | https://github.com/mystor/synstructure | Copyright 2016 Nika Layzell |
| sys-locale | 0.3.2 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/1Password/sys-locale | Copyright (c) 2021 1Password |
| system-configuration | 0.7.0 | macOS | MIT OR Apache-2.0 | MIT | https://github.com/mullvad/system-configuration-rs | Copyright (c) 2024 Mullvad VPN AB |
| system-configuration-sys | 0.6.0 | macOS | MIT OR Apache-2.0 | MIT | https://github.com/mullvad/system-configuration-rs | Copyright (c) 2024 Mullvad VPN AB |
| tao | 0.35.3 | 全部 | Apache-2.0 | Apache-2.0 | https://github.com/tauri-apps/tao | 作者：Tauri Programme within The Commons Conservancy、The winit contributors |
| tap | 1.0.1 | 全部 | MIT | MIT | https://github.com/myrrlyn/tap | Copyright (c) 2017 Elliot Linder <darfink@gmail.com> |
| tauri | 2.11.6 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/tauri-apps/tauri | Copyright (c) 2017 - Present Tauri Apps Contributors |
| tauri-codegen | 2.6.3 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/tauri-apps/tauri | Copyright (c) 2017 - Present Tauri Apps Contributors |
| tauri-macros | 2.6.3 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/tauri-apps/tauri | Copyright (c) 2017 - Present Tauri Apps Contributors |
| tauri-plugin-dialog | 2.7.3 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/tauri-apps/plugins-workspace | Copyright (c) 2017 - Present Tauri Apps Contributors |
| tauri-plugin-fs | 2.5.2 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/tauri-apps/plugins-workspace | Copyright (c) 2017 - Present Tauri Apps Contributors |
| tauri-plugin-opener | 2.5.5 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/tauri-apps/plugins-workspace | Copyright (c) 2017 - Present Tauri Apps Contributors |
| tauri-plugin-single-instance | 2.4.5 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/tauri-apps/plugins-workspace | Copyright (c) 2017 - Present The Tauri Programme in the Commons Conservancy |
| tauri-runtime | 2.11.3 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/tauri-apps/tauri | Copyright (c) 2017 - Present Tauri Apps Contributors |
| tauri-runtime-wry | 2.11.4 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/tauri-apps/tauri | Copyright (c) 2017 - Present Tauri Apps Contributors |
| tauri-utils | 2.9.3 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/tauri-apps/tauri | Copyright (c) 2017 - Present Tauri Apps Contributors |
| tendril | 0.5.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/servo/html5ever | Copyright (c) 2015 Keegan McAllister |
| thiserror | 1.0.69 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/thiserror | 作者：David Tolnay |
| thiserror | 2.0.21 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/thiserror | 作者：David Tolnay |
| thiserror-impl | 1.0.69 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/thiserror | 作者：David Tolnay |
| thiserror-impl | 2.0.21 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/thiserror | 作者：David Tolnay |
| thread_local | 1.1.10 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/Amanieu/thread_local-rs | Copyright (c) 2016 The Rust Project Developers |
| tiff | 0.11.3 | macOS | MIT | MIT | https://github.com/image-rs/image-tiff | Copyright (c) 2018 PistonDevelopers |
| time | 0.3.55 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/time-rs/time | Copyright (c) Jacob Pratt et al. |
| time-core | 0.1.9 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/time-rs/time | Copyright (c) Jacob Pratt et al. |
| time-macros | 0.2.32 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/time-rs/time | Copyright (c) Jacob Pratt et al. |
| tinystr | 0.8.4 | 全部 | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| tokio | 1.53.1 | 全部 | MIT | MIT | https://github.com/tokio-rs/tokio | Copyright (c) Tokio Contributors |
| tokio-macros | 2.7.2 | 全部 | MIT | MIT | https://github.com/tokio-rs/tokio | Copyright (c) 2019 Yoshua Wuyts；Copyright (c) Tokio Contributors |
| tokio-rustls | 0.26.5 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/rustls/tokio-rustls | Copyright 2017 quininer kel；Copyright (c) 2017 quininer kel |
| tokio-tungstenite | 0.29.0 | 全部 | MIT | MIT | https://github.com/snapview/tokio-tungstenite | Copyright (c) 2017 Daniel Abramov；Copyright (c) 2017 Alexey Galakhov |
| tokio-util | 0.7.19 | 全部 | MIT | MIT | https://github.com/tokio-rs/tokio | Copyright (c) Tokio Contributors |
| toml | 1.1.6+spec-1.1.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/toml-rs/toml | Copyright (c) Individual contributors |
| toml_datetime | 0.6.3 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/toml-rs/toml | Copyright (c) 2014 Alex Crichton |
| toml_datetime | 1.1.1+spec-1.1.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/toml-rs/toml | Copyright (c) Individual contributors |
| toml_edit | 0.19.15 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/toml-rs/toml | Copyright (c) Individual contributors |
| toml_edit | 0.20.2 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/toml-rs/toml | Copyright (c) Individual contributors |
| toml_edit | 0.25.15+spec-1.1.0 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/toml-rs/toml | Copyright (c) Individual contributors |
| toml_parser | 1.1.3+spec-1.1.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/toml-rs/toml | Copyright (c) Individual contributors |
| toml_writer | 1.1.2+spec-1.1.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/toml-rs/toml | Copyright (c) Individual contributors |
| tower | 0.5.3 | 全部 | MIT | MIT | https://github.com/tower-rs/tower | Copyright (c) 2019 Tower Contributors |
| tower-http | 0.6.11 | 全部 | MIT | MIT | https://github.com/tower-rs/tower-http | Copyright (c) 2019-2021 Tower Contributors |
| tower-layer | 0.3.3 | 全部 | MIT | MIT | https://github.com/tower-rs/tower | Copyright (c) 2019 Tower Contributors |
| tower-service | 0.3.3 | 全部 | MIT | MIT | https://github.com/tower-rs/tower | Copyright (c) 2019 Tower Contributors |
| tracing | 0.1.44 | 全部 | MIT | MIT | https://github.com/tokio-rs/tracing | Copyright (c) 2019 Tokio Contributors |
| tracing-attributes | 0.1.31 | 全部 | MIT | MIT | https://github.com/tokio-rs/tracing | Copyright (c) 2019 Tokio Contributors |
| tracing-core | 0.1.36 | 全部 | MIT | MIT | https://github.com/tokio-rs/tracing | Copyright (c) 2019 Tokio Contributors |
| tracing-log | 0.2.0 | 全部 | MIT | MIT | https://github.com/tokio-rs/tracing | Copyright (c) 2019 Tokio Contributors |
| tracing-subscriber | 0.3.23 | 全部 | MIT | MIT | https://github.com/tokio-rs/tracing | Copyright (c) 2019 Tokio Contributors |
| trash | 5.2.9 | 全部 | MIT | MIT | https://github.com/ArturKovacs/trash | Copyright 2019 Artúr Barnabás Kovács |
| try-lock | 0.2.5 | 全部 | MIT | MIT | https://github.com/seanmonstar/try-lock | Copyright (c) 2018-2023 Sean McArthur；Copyright (c) 2016 Alex Crichton |
| tungstenite | 0.29.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/snapview/tungstenite-rs | Copyright (c) 2017 Alexey Galakhov；Copyright (c) 2016 Jason Housley |
| twofish | 0.7.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-ciphers | Copyright (c) 2017 Alexander Krotov |
| typeid | 1.0.3 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/typeid | 作者：David Tolnay |
| typenum | 1.20.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/paholg/typenum | Copyright 2014 Paho Lurie-Gregg；Copyright (c) 2014 Paho Lurie-Gregg |
| unic-char-property | 0.9.0 | 全部 | MIT/Apache-2.0 | MIT | https://github.com/open-i18n/rust-unic/ | 作者：The UNIC Project Developers |
| unic-char-range | 0.9.0 | 全部 | MIT/Apache-2.0 | MIT | https://github.com/open-i18n/rust-unic/ | 作者：The UNIC Project Developers |
| unic-common | 0.9.0 | 全部 | MIT/Apache-2.0 | MIT | https://github.com/open-i18n/rust-unic/ | 作者：The UNIC Project Developers |
| unic-ucd-ident | 0.9.0 | 全部 | MIT/Apache-2.0 | MIT | https://github.com/open-i18n/rust-unic/ | 作者：The UNIC Project Developers |
| unic-ucd-version | 0.9.0 | 全部 | MIT/Apache-2.0 | MIT | https://github.com/open-i18n/rust-unic/ | 作者：The UNIC Project Developers |
| unicode-ident | 1.0.26 | 全部 | (MIT OR Apache-2.0) AND Unicode-3.0 | MIT + Unicode-3.0 | https://github.com/dtolnay/unicode-ident | Copyright © 1991-2023 Unicode, Inc. |
| unicode-segmentation | 1.13.3 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/unicode-rs/unicode-segmentation | Copyright (c) 2015 The Rust Project Developers |
| unicode-xid | 0.2.6 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/unicode-rs/unicode-xid | Copyright (c) 2015 The Rust Project Developers |
| universal-hash | 0.5.1 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/traits | Copyright (c) 2019-2020 RustCrypto Developers |
| untrusted | 0.9.0 | 全部 | ISC | ISC | https://github.com/briansmith/untrusted | Copyright 2015-2016 Brian Smith. |
| url | 2.5.8 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/servo/rust-url | Copyright (c) 2013-2025 The rust-url developers |
| urlencoding | 2.1.3 | Linux | MIT | MIT | https://github.com/kornelski/rust_urlencoding | © 2016 Bertram Truong；© 2021 Kornel Lesiński |
| urlpattern | 0.3.0 | 全部 | MIT | MIT | https://github.com/denoland/rust-urlpattern | Copyright (c) 2021 the Deno authors |
| utf8_iter | 1.0.4 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/hsivonen/utf8_iter | Copyright Mozilla Foundation |
| uuid | 1.26.1 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/uuid-rs/uuid | Copyright (c) 2014 The Rust Project Developers；Copyright (c) 2018 Ashley Mannix, Christopher Armstrong, Dylan DPC, Hunar Roop Kahlon |
| walkdir | 2.5.0 | 全部 | Unlicense/MIT | MIT | https://github.com/BurntSushi/walkdir | Copyright (c) 2015 Andrew Gallant |
| want | 0.3.1 | 全部 | MIT | MIT | https://github.com/seanmonstar/want | Copyright (c) 2018-2019 Sean McArthur |
| web_atoms | 0.2.6 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/servo/html5ever | Copyright (c) 2014 The html5ever Project Developers |
| webkit2gtk | 2.0.2 | Linux | MIT | MIT | https://github.com/tauri-apps/webkit2gtk-rs | Copyright (c) 2016 Boucher, Antoni <bouanto@zoho.com>；Copyright (c) 2017-2021, The Gtk-rs Project Developers.；Copyright (c) 2021, Tauri Programme within The Commons Conservancy |
| webkit2gtk-sys | 2.0.2 | Linux | MIT | MIT | https://github.com/tauri-apps/webkit2gtk-rs | Copyright (c) 2016 Boucher, Antoni <bouanto@zoho.com> |
| webpki-roots | 1.0.9 | 全部 | CDLA-Permissive-2.0 | CDLA-Permissive-2.0 | https://github.com/rustls/webpki-roots | — |
| webview2-com | 0.38.2 | Windows | MIT | MIT | https://github.com/wravery/webview2-rs | — |
| webview2-com-macros | 0.8.1 | Windows | MIT | MIT | https://github.com/wravery/webview2-rs | — |
| webview2-com-sys | 0.38.2 | Windows | MIT | MIT | https://github.com/wravery/webview2-rs | — |
| weezl | 0.1.12 | macOS | MIT OR Apache-2.0 | MIT | https://github.com/image-rs/weezl | Copyright (c) HeroicKatora 2020 |
| winapi-util | 0.1.11 | Windows | Unlicense OR MIT | MIT | https://github.com/BurntSushi/winapi-util | Copyright (c) 2017 Andrew Gallant |
| window-vibrancy | 0.6.0 | macOS + Windows | Apache-2.0 OR MIT | MIT | https://github.com/tauri-apps/tauri-plugin-vibrancy | Copyright (c) 2020-2022 Tauri Programme within The Commons Conservancy |
| windows | 0.59.0 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows | 0.61.3 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows | 0.62.2 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-collections | 0.2.0 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-collections | 0.3.2 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-core | 0.59.0 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-core | 0.61.2 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-core | 0.62.2 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-future | 0.2.1 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-future | 0.3.2 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-implement | 0.59.0 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-implement | 0.60.2 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-interface | 0.59.3 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-link | 0.1.3 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-link | 0.2.1 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-numerics | 0.2.0 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-numerics | 0.3.1 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-result | 0.3.4 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-result | 0.4.1 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-strings | 0.3.1 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-strings | 0.4.2 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-strings | 0.5.1 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-sys | 0.59.0 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-sys | 0.60.2 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-sys | 0.61.2 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-targets | 0.52.6 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-targets | 0.53.5 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-threading | 0.1.0 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-threading | 0.2.1 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-version | 0.1.7 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows-win | 3.0.0 | Windows | BSL-1.0 | BSL-1.0 | https://github.com/DoumanAsh/windows-win-rs | 作者：Douman |
| windows_x86_64_msvc | 0.52.6 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| windows_x86_64_msvc | 0.53.1 | Windows | MIT OR Apache-2.0 | MIT | https://github.com/microsoft/windows-rs | Copyright (c) Microsoft Corporation. |
| winnow | 0.5.40 | Linux | MIT | MIT | https://github.com/winnow-rs/winnow | — |
| winnow | 1.0.4 | 全部 | MIT | MIT | https://github.com/winnow-rs/winnow | — |
| winreg | 0.55.0 | Windows | MIT | MIT | https://github.com/gentoo90/winreg-rs | Copyright (c) 2015 Igor Shaula |
| writeable | 0.6.4 | 全部 | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| wry | 0.55.1 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/tauri-apps/wry | Copyright (c) 2020-2023 Ngo Iok Ui & Tauri Programme within The Commons Conservancy |
| wyz | 0.5.1 | 全部 | MIT | MIT | https://github.com/myrrlyn/wyz | Copyright (c) 2018 myrrlyn (Alexander Payne) |
| x11 | 2.21.0 | Linux | MIT | MIT | https://github.com/AltF02/x11-rs.git | 作者：daggerbot、Erle Pereira、AltF02 |
| x11-dl | 2.21.0 | Linux | MIT | MIT | https://github.com/AltF02/x11-rs.git | 作者：daggerbot、Erle Pereira、AltF02 |
| x11rb | 0.13.2 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/psychon/x11rb | Copyright 2019 x11rb Contributers |
| x11rb-protocol | 0.13.2 | Linux | MIT OR Apache-2.0 | MIT | https://github.com/psychon/x11rb | Copyright 2019 x11rb Contributers |
| x25519-dalek | 2.0.1 | 全部 | BSD-3-Clause | BSD-3-Clause | https://github.com/dalek-cryptography/curve25519-dalek/tree/main/x25519-dalek | Copyright (c) 2017-2021 isis agora lovecruft. All rights reserved.；Copyright (c) 2019-2021 DebugSteven. All rights reserved. |
| yasna | 0.6.0 | 全部 | MIT OR Apache-2.0 | MIT | https://github.com/qnighy/yasna.rs | Copyright (c) 2016 Masaki Hara |
| yoke | 0.8.3 | 全部 | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| yoke-derive | 0.8.3 | 全部 | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| zbus | 5.19.0 | Linux | MIT | MIT | https://github.com/z-galaxy/zbus/ | Copyright (c) 2024 Zeeshan Ali Khan & zbus contributors |
| zbus_macros | 5.19.0 | Linux | MIT | MIT | https://github.com/z-galaxy/zbus/ | Copyright (c) 2024 Zeeshan Ali Khan & zbus contributors |
| zbus_names | 4.3.4 | Linux | MIT | MIT | https://github.com/z-galaxy/zbus/ | Copyright (c) 2024 Zeeshan Ali Khan & zbus contributors |
| zcheapstr | 1.1.0 | Linux | MIT | MIT | https://github.com/z-galaxy/zcheapstr/ | Copyright (c) 2026 Zeeshan Ali Khan & zcheapstr contributors |
| zerocopy | 0.8.59 | 全部 | BSD-2-Clause OR Apache-2.0 OR MIT | MIT | https://github.com/google/zerocopy | Copyright 2023 The Fuchsia Authors；Copyright 2019 The Fuchsia Authors. |
| zerocopy-derive | 0.8.59 | macOS | BSD-2-Clause OR Apache-2.0 OR MIT | MIT | https://github.com/google/zerocopy | Copyright 2023 The Fuchsia Authors；Copyright 2019 The Fuchsia Authors. |
| zerofrom | 0.1.8 | 全部 | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| zerofrom-derive | 0.1.8 | 全部 | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| zeroize | 1.9.0 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/utils | Copyright (c) 2018-2026 The RustCrypto Project Developers |
| zeroize_derive | 1.5.0 | 全部 | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/utils | Copyright (c) 2019-2026 The RustCrypto Project Developers |
| zerotrie | 0.2.5 | 全部 | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| zerovec | 0.11.8 | 全部 | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| zerovec-derive | 0.11.6 | 全部 | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| zip | 4.6.1 | 全部 | MIT | MIT | https://github.com/zip-rs/zip2.git | Copyright (c) 2014 Mathijs van de Nes |
| zlib-rs | 0.6.8 | 全部 | Zlib | Zlib | https://github.com/trifectatechfoundation/zlib-rs | (C) 2024 Trifecta Tech Foundation |
| zmij | 1.0.23 | 全部 | MIT | MIT | https://github.com/dtolnay/zmij | 作者：David Tolnay |
| zopfli | 0.8.4 | 全部 | Apache-2.0 | Apache-2.0 | https://github.com/zopfli-rs/zopfli | Copyright 2011 Google Inc. |
| zune-core | 0.5.3 | 全部 | MIT OR Apache-2.0 OR Zlib | MIT | https://github.com/etemesi254/zune-image | Copyright (c) zune-image developers |
| zune-jpeg | 0.5.15 | 全部 | MIT OR Apache-2.0 OR Zlib | MIT | https://github.com/etemesi254/zune-image/tree/dev/crates/zune-jpeg | Copyright (c) zune-image developers |
| zvariant | 5.15.0 | Linux | MIT | MIT | https://github.com/z-galaxy/zbus/ | Copyright (c) 2024 Zeeshan Ali Khan & zbus contributors |
| zvariant_derive | 5.15.0 | Linux | MIT | MIT | https://github.com/z-galaxy/zbus/ | Copyright (c) 2024 Zeeshan Ali Khan & zbus contributors |
| zvariant_utils | 4.2.0 | Linux | MIT | MIT | https://github.com/z-galaxy/zbus/ | 作者：Zeeshan Ali Khan、turbocooler |

## MIT 许可原文

（各 crate 的版权行见上表）

```
Permission is hereby granted, free of charge, to any
person obtaining a copy of this software and associated
documentation files (the "Software"), to deal in the
Software without restriction, including without
limitation the rights to use, copy, modify, merge,
publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software
is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice
shall be included in all copies or substantial portions
of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
```

## Apache License 2.0 原文

```
Apache License
                        Version 2.0, January 2004
                     https://www.apache.org/licenses/LICENSE-2.0

TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION

1. Definitions.

   "License" shall mean the terms and conditions for use, reproduction,
   and distribution as defined by Sections 1 through 9 of this document.

   "Licensor" shall mean the copyright owner or entity authorized by
   the copyright owner that is granting the License.

   "Legal Entity" shall mean the union of the acting entity and all
   other entities that control, are controlled by, or are under common
   control with that entity. For the purposes of this definition,
   "control" means (i) the power, direct or indirect, to cause the
   direction or management of such entity, whether by contract or
   otherwise, or (ii) ownership of fifty percent (50%) or more of the
   outstanding shares, or (iii) beneficial ownership of such entity.

   "You" (or "Your") shall mean an individual or Legal Entity
   exercising permissions granted by this License.

   "Source" form shall mean the preferred form for making modifications,
   including but not limited to software source code, documentation
   source, and configuration files.

   "Object" form shall mean any form resulting from mechanical
   transformation or translation of a Source form, including but
   not limited to compiled object code, generated documentation,
   and conversions to other media types.

   "Work" shall mean the work of authorship, whether in Source or
   Object form, made available under the License, as indicated by a
   copyright notice that is included in or attached to the work
   (an example is provided in the Appendix below).

   "Derivative Works" shall mean any work, whether in Source or Object
   form, that is based on (or derived from) the Work and for which the
   editorial revisions, annotations, elaborations, or other modifications
   represent, as a whole, an original work of authorship. For the purposes
   of this License, Derivative Works shall not include works that remain
   separable from, or merely link (or bind by name) to the interfaces of,
   the Work and Derivative Works thereof.

   "Contribution" shall mean any work of authorship, including
   the original version of the Work and any modifications or additions
   to that Work or Derivative Works thereof, that is intentionally
   submitted to Licensor for inclusion in the Work by the copyright owner
   or by an individual or Legal Entity authorized to submit on behalf of
   the copyright owner. For the purposes of this definition, "submitted"
   means any form of electronic, verbal, or written communication sent
   to the Licensor or its representatives, including but not limited to
   communication on electronic mailing lists, source code control systems,
   and issue tracking systems that are managed by, or on behalf of, the
   Licensor for the purpose of discussing and improving the Work, but
   excluding communication that is conspicuously marked or otherwise
   designated in writing by the copyright owner as "Not a Contribution."

   "Contributor" shall mean Licensor and any individual or Legal Entity
   on behalf of whom a Contribution has been received by Licensor and
   subsequently incorporated within the Work.

2. Grant of Copyright License. Subject to the terms and conditions of
   this License, each Contributor hereby grants to You a perpetual,
   worldwide, non-exclusive, no-charge, royalty-free, irrevocable
   copyright license to reproduce, prepare Derivative Works of,
   publicly display, publicly perform, sublicense, and distribute the
   Work and such Derivative Works in Source or Object form.

3. Grant of Patent License. Subject to the terms and conditions of
   this License, each Contributor hereby grants to You a perpetual,
   worldwide, non-exclusive, no-charge, royalty-free, irrevocable
   (except as stated in this section) patent license to make, have made,
   use, offer to sell, sell, import, and otherwise transfer the Work,
   where such license applies only to those patent claims licensable
   by such Contributor that are necessarily infringed by their
   Contribution(s) alone or by combination of their Contribution(s)
   with the Work to which such Contribution(s) was submitted. If You
   institute patent litigation against any entity (including a
   cross-claim or counterclaim in a lawsuit) alleging that the Work
   or a Contribution incorporated within the Work constitutes direct
   or contributory patent infringement, then any patent licenses
   granted to You under this License for that Work shall terminate
   as of the date such litigation is filed.

4. Redistribution. You may reproduce and distribute copies of the
   Work or Derivative Works thereof in any medium, with or without
   modifications, and in Source or Object form, provided that You
   meet the following conditions:

   (a) You must give any other recipients of the Work or
       Derivative Works a copy of this License; and

   (b) You must cause any modified files to carry prominent notices
       stating that You changed the files; and

   (c) You must retain, in the Source form of any Derivative Works
       that You distribute, all copyright, patent, trademark, and
       attribution notices from the Source form of the Work,
       excluding those notices that do not pertain to any part of
       the Derivative Works; and

   (d) If the Work includes a "NOTICE" text file as part of its
       distribution, then any Derivative Works that You distribute must
       include a readable copy of the attribution notices contained
       within such NOTICE file, excluding those notices that do not
       pertain to any part of the Derivative Works, in at least one
       of the following places: within a NOTICE text file distributed
       as part of the Derivative Works; within the Source form or
       documentation, if provided along with the Derivative Works; or,
       within a display generated by the Derivative Works, if and
       wherever such third-party notices normally appear. The contents
       of the NOTICE file are for informational purposes only and
       do not modify the License. You may add Your own attribution
       notices within Derivative Works that You distribute, alongside
       or as an addendum to the NOTICE text from the Work, provided
       that such additional attribution notices cannot be construed
       as modifying the License.

   You may add Your own copyright statement to Your modifications and
   may provide additional or different license terms and conditions
   for use, reproduction, or distribution of Your modifications, or
   for any such Derivative Works as a whole, provided Your use,
   reproduction, and distribution of the Work otherwise complies with
   the conditions stated in this License.

5. Submission of Contributions. Unless You explicitly state otherwise,
   any Contribution intentionally submitted for inclusion in the Work
   by You to the Licensor shall be under the terms and conditions of
   this License, without any additional terms or conditions.
   Notwithstanding the above, nothing herein shall supersede or modify
   the terms of any separate license agreement you may have executed
   with Licensor regarding such Contributions.

6. Trademarks. This License does not grant permission to use the trade
   names, trademarks, service marks, or product names of the Licensor,
   except as required for reasonable and customary use in describing the
   origin of the Work and reproducing the content of the NOTICE file.

7. Disclaimer of Warranty. Unless required by applicable law or
   agreed to in writing, Licensor provides the Work (and each
   Contributor provides its Contributions) on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or
   implied, including, without limitation, any warranties or conditions
   of TITLE, NON-INFRINGEMENT, MERCHANTABILITY, or FITNESS FOR A
   PARTICULAR PURPOSE. You are solely responsible for determining the
   appropriateness of using or redistributing the Work and assume any
   risks associated with Your exercise of permissions under this License.

8. Limitation of Liability. In no event and under no legal theory,
   whether in tort (including negligence), contract, or otherwise,
   unless required by applicable law (such as deliberate and grossly
   negligent acts) or agreed to in writing, shall any Contributor be
   liable to You for damages, including any direct, indirect, special,
   incidental, or consequential damages of any character arising as a
   result of this License or out of the use or inability to use the
   Work (including but not limited to damages for loss of goodwill,
   work stoppage, computer failure or malfunction, or any and all
   other commercial damages or losses), even if such Contributor
   has been advised of the possibility of such damages.

9. Accepting Warranty or Additional Liability. While redistributing
   the Work or Derivative Works thereof, You may choose to offer,
   and charge a fee for, acceptance of support, warranty, indemnity,
   or other liability obligations and/or rights consistent with this
   License. However, in accepting such obligations, You may act only
   on Your own behalf and on Your sole responsibility, not on behalf
   of any other Contributor, and only if You agree to indemnify,
   defend, and hold each Contributor harmless for any liability
   incurred by, or claims asserted against, such Contributor by reason
   of your accepting any such warranty or additional liability.

END OF TERMS AND CONDITIONS
```

## 其余许可证：各 crate 自带的许可证文件（原样）

### alloc-no-stdlib 2.0.4（LICENSE）、brotli 8.0.4（LICENSE.BSD-3-Clause）

```
Copyright (c) 2016 Dropbox, Inc.
All rights reserved.

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its contributors may be used to endorse or promote products derived from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

### alloc-stdlib 0.2.4、cx448 0.1.1

```
这些包没有附许可证文件，许可证是 BSD-3-Clause，标准文本见 https://spdx.org/licenses/BSD-3-Clause.html ，版权归各自的作者（见上表）。
```

### clipboard-win 5.4.1

```
这些包没有附许可证文件，许可证是 BSL-1.0，标准文本见 https://spdx.org/licenses/BSL-1.0.html ，版权归各自的作者（见上表）。
```

### cssparser 0.36.0（LICENSE）、cssparser-macros 0.6.1（LICENSE）、dtoa-short 0.3.5（LICENSE）

```
Mozilla Public License Version 2.0
==================================

1. Definitions
--------------

1.1. "Contributor"
    means each individual or legal entity that creates, contributes to
    the creation of, or owns Covered Software.

1.2. "Contributor Version"
    means the combination of the Contributions of others (if any) used
    by a Contributor and that particular Contributor's Contribution.

1.3. "Contribution"
    means Covered Software of a particular Contributor.

1.4. "Covered Software"
    means Source Code Form to which the initial Contributor has attached
    the notice in Exhibit A, the Executable Form of such Source Code
    Form, and Modifications of such Source Code Form, in each case
    including portions thereof.

1.5. "Incompatible With Secondary Licenses"
    means

    (a) that the initial Contributor has attached the notice described
        in Exhibit B to the Covered Software; or

    (b) that the Covered Software was made available under the terms of
        version 1.1 or earlier of the License, but not also under the
        terms of a Secondary License.

1.6. "Executable Form"
    means any form of the work other than Source Code Form.

1.7. "Larger Work"
    means a work that combines Covered Software with other material, in 
    a separate file or files, that is not Covered Software.

1.8. "License"
    means this document.

1.9. "Licensable"
    means having the right to grant, to the maximum extent possible,
    whether at the time of the initial grant or subsequently, any and
    all of the rights conveyed by this License.

1.10. "Modifications"
    means any of the following:

    (a) any file in Source Code Form that results from an addition to,
        deletion from, or modification of the contents of Covered
        Software; or

    (b) any new file in Source Code Form that contains any Covered
        Software.

1.11. "Patent Claims" of a Contributor
    means any patent claim(s), including without limitation, method,
    process, and apparatus claims, in any patent Licensable by such
    Contributor that would be infringed, but for the grant of the
    License, by the making, using, selling, offering for sale, having
    made, import, or transfer of either its Contributions or its
    Contributor Version.

1.12. "Secondary License"
    means either the GNU General Public License, Version 2.0, the GNU
    Lesser General Public License, Version 2.1, the GNU Affero General
    Public License, Version 3.0, or any later versions of those
    licenses.

1.13. "Source Code Form"
    means the form of the work preferred for making modifications.

1.14. "You" (or "Your")
    means an individual or a legal entity exercising rights under this
    License. For legal entities, "You" includes any entity that
    controls, is controlled by, or is under common control with You. For
    purposes of this definition, "control" means (a) the power, direct
    or indirect, to cause the direction or management of such entity,
    whether by contract or otherwise, or (b) ownership of more than
    fifty percent (50%) of the outstanding shares or beneficial
    ownership of such entity.

2. License Grants and Conditions
--------------------------------

2.1. Grants

Each Contributor hereby grants You a world-wide, royalty-free,
non-exclusive license:

(a) under intellectual property rights (other than patent or trademark)
    Licensable by such Contributor to use, reproduce, make available,
    modify, display, perform, distribute, and otherwise exploit its
    Contributions, either on an unmodified basis, with Modifications, or
    as part of a Larger Work; and

(b) under Patent Claims of such Contributor to make, use, sell, offer
    for sale, have made, import, and otherwise transfer either its
    Contributions or its Contributor Version.

2.2. Effective Date

The licenses granted in Section 2.1 with respect to any Contribution
become effective for each Contribution on the date the Contributor first
distributes such Contribution.

2.3. Limitations on Grant Scope

The licenses granted in this Section 2 are the only rights granted under
this License. No additional rights or licenses will be implied from the
distribution or licensing of Covered Software under this License.
Notwithstanding Section 2.1(b) above, no patent license is granted by a
Contributor:

(a) for any code that a Contributor has removed from Covered Software;
    or

(b) for infringements caused by: (i) Your and any other third party's
    modifications of Covered Software, or (ii) the combination of its
    Contributions with other software (except as part of its Contributor
    Version); or

(c) under Patent Claims infringed by Covered Software in the absence of
    its Contributions.

This License does not grant any rights in the trademarks, service marks,
or logos of any Contributor (except as may be necessary to comply with
the notice requirements in Section 3.4).

2.4. Subsequent Licenses

No Contributor makes additional grants as a result of Your choice to
distribute the Covered Software under a subsequent version of this
License (see Section 10.2) or under the terms of a Secondary License (if
permitted under the terms of Section 3.3).

2.5. Representation

Each Contributor represents that the Contributor believes its
Contributions are its original creation(s) or it has sufficient rights
to grant the rights to its Contributions conveyed by this License.

2.6. Fair Use

This License is not intended to limit any rights You have under
applicable copyright doctrines of fair use, fair dealing, or other
equivalents.

2.7. Conditions

Sections 3.1, 3.2, 3.3, and 3.4 are conditions of the licenses granted
in Section 2.1.

3. Responsibilities
-------------------

3.1. Distribution of Source Form

All distribution of Covered Software in Source Code Form, including any
Modifications that You create or to which You contribute, must be under
the terms of this License. You must inform recipients that the Source
Code Form of the Covered Software is governed by the terms of this
License, and how they can obtain a copy of this License. You may not
attempt to alter or restrict the recipients' rights in the Source Code
Form.

3.2. Distribution of Executable Form

If You distribute Covered Software in Executable Form then:

(a) such Covered Software must also be made available in Source Code
    Form, as described in Section 3.1, and You must inform recipients of
    the Executable Form how they can obtain a copy of such Source Code
    Form by reasonable means in a timely manner, at a charge no more
    than the cost of distribution to the recipient; and

(b) You may distribute such Executable Form under the terms of this
    License, or sublicense it under different terms, provided that the
    license for the Executable Form does not attempt to limit or alter
    the recipients' rights in the Source Code Form under this License.

3.3. Distribution of a Larger Work

You may create and distribute a Larger Work under terms of Your choice,
provided that You also comply with the requirements of this License for
the Covered Software. If the Larger Work is a combination of Covered
Software with a work governed by one or more Secondary Licenses, and the
Covered Software is not Incompatible With Secondary Licenses, this
License permits You to additionally distribute such Covered Software
under the terms of such Secondary License(s), so that the recipient of
the Larger Work may, at their option, further distribute the Covered
Software under the terms of either this License or such Secondary
License(s).

3.4. Notices

You may not remove or alter the substance of any license notices
(including copyright notices, patent notices, disclaimers of warranty,
or limitations of liability) contained within the Source Code Form of
the Covered Software, except that You may alter any license notices to
the extent required to remedy known factual inaccuracies.

3.5. Application of Additional Terms

You may choose to offer, and to charge a fee for, warranty, support,
indemnity or liability obligations to one or more recipients of Covered
Software. However, You may do so only on Your own behalf, and not on
behalf of any Contributor. You must make it absolutely clear that any
such warranty, support, indemnity, or liability obligation is offered by
You alone, and You hereby agree to indemnify every Contributor for any
liability incurred by such Contributor as a result of warranty, support,
indemnity or liability terms You offer. You may include additional
disclaimers of warranty and limitations of liability specific to any
jurisdiction.

4. Inability to Comply Due to Statute or Regulation
---------------------------------------------------

If it is impossible for You to comply with any of the terms of this
License with respect to some or all of the Covered Software due to
statute, judicial order, or regulation then You must: (a) comply with
the terms of this License to the maximum extent possible; and (b)
describe the limitations and the code they affect. Such description must
be placed in a text file included with all distributions of the Covered
Software under this License. Except to the extent prohibited by statute
or regulation, such description must be sufficiently detailed for a
recipient of ordinary skill to be able to understand it.

5. Termination
--------------

5.1. The rights granted under this License will terminate automatically
if You fail to comply with any of its terms. However, if You become
compliant, then the rights granted under this License from a particular
Contributor are reinstated (a) provisionally, unless and until such
Contributor explicitly and finally terminates Your grants, and (b) on an
ongoing basis, if such Contributor fails to notify You of the
non-compliance by some reasonable means prior to 60 days after You have
come back into compliance. Moreover, Your grants from a particular
Contributor are reinstated on an ongoing basis if such Contributor
notifies You of the non-compliance by some reasonable means, this is the
first time You have received notice of non-compliance with this License
from such Contributor, and You become compliant prior to 30 days after
Your receipt of the notice.

5.2. If You initiate litigation against any entity by asserting a patent
infringement claim (excluding declaratory judgment actions,
counter-claims, and cross-claims) alleging that a Contributor Version
directly or indirectly infringes any patent, then the rights granted to
You by any and all Contributors for the Covered Software under Section
2.1 of this License shall terminate.

5.3. In the event of termination under Sections 5.1 or 5.2 above, all
end user license agreements (excluding distributors and resellers) which
have been validly granted by You or Your distributors under this License
prior to termination shall survive termination.

************************************************************************
*                                                                      *
*  6. Disclaimer of Warranty                                           *
*  -------------------------                                           *
*                                                                      *
*  Covered Software is provided under this License on an "as is"       *
*  basis, without warranty of any kind, either expressed, implied, or  *
*  statutory, including, without limitation, warranties that the       *
*  Covered Software is free of defects, merchantable, fit for a        *
*  particular purpose or non-infringing. The entire risk as to the     *
*  quality and performance of the Covered Software is with You.        *
*  Should any Covered Software prove defective in any respect, You     *
*  (not any Contributor) assume the cost of any necessary servicing,   *
*  repair, or correction. This disclaimer of warranty constitutes an   *
*  essential part of this License. No use of any Covered Software is   *
*  authorized under this License except under this disclaimer.         *
*                                                                      *
************************************************************************

************************************************************************
*                                                                      *
*  7. Limitation of Liability                                          *
*  --------------------------                                          *
*                                                                      *
*  Under no circumstances and under no legal theory, whether tort      *
*  (including negligence), contract, or otherwise, shall any           *
*  Contributor, or anyone who distributes Covered Software as          *
*  permitted above, be liable to You for any direct, indirect,         *
*  special, incidental, or consequential damages of any character      *
*  including, without limitation, damages for lost profits, loss of    *
*  goodwill, work stoppage, computer failure or malfunction, or any    *
*  and all other commercial damages or losses, even if such party      *
*  shall have been informed of the possibility of such damages. This   *
*  limitation of liability shall not apply to liability for death or   *
*  personal injury resulting from such party's negligence to the       *
*  extent applicable law prohibits such limitation. Some               *
*  jurisdictions do not allow the exclusion or limitation of           *
*  incidental or consequential damages, so this exclusion and          *
*  limitation may not apply to You.                                    *
*                                                                      *
************************************************************************

8. Litigation
-------------

Any litigation relating to this License may be brought only in the
courts of a jurisdiction where the defendant maintains its principal
place of business and such litigation shall be governed by laws of that
jurisdiction, without reference to its conflict-of-law provisions.
Nothing in this Section shall prevent a party's ability to bring
cross-claims or counter-claims.

9. Miscellaneous
----------------

This License represents the complete agreement concerning the subject
matter hereof. If any provision of this License is held to be
unenforceable, such provision shall be reformed only to the extent
necessary to make it enforceable. Any law or regulation which provides
that the language of a contract shall be construed against the drafter
shall not be used to construe this License against a Contributor.

10. Versions of the License
---------------------------

10.1. New Versions

Mozilla Foundation is the license steward. Except as provided in Section
10.3, no one other than the license steward has the right to modify or
publish new versions of this License. Each version will be given a
distinguishing version number.

10.2. Effect of New Versions

You may distribute the Covered Software under the terms of the version
of the License under which You originally received the Covered Software,
or under the terms of any subsequent version published by the license
steward.

10.3. Modified Versions

If you create software not governed by this License, and you want to
create a new license for such software, you may create and use a
modified version of this License if you rename the license and remove
any references to the name of the license steward (except to note that
such modified license differs from this License).

10.4. Distributing Source Code Form that is Incompatible With Secondary
Licenses

If You choose to distribute Source Code Form that is Incompatible With
Secondary Licenses under the terms of this version of the License, the
notice described in Exhibit B of this License must be attached.

Exhibit A - Source Code Form License Notice
-------------------------------------------

  This Source Code Form is subject to the terms of the Mozilla Public
  License, v. 2.0. If a copy of the MPL was not distributed with this
  file, You can obtain one at http://mozilla.org/MPL/2.0/.

If it is not possible or desirable to put the notice in a particular
file, then You may include the notice in a location (such as a LICENSE
file in a relevant directory) where a recipient would be likely to look
for such a notice.

You may add additional accurate notices of copyright ownership.

Exhibit B - "Incompatible With Secondary Licenses" Notice
---------------------------------------------------------

  This Source Code Form is "Incompatible With Secondary Licenses", as
  defined by the Mozilla Public License, v. 2.0.
```

### curve25519-dalek 4.1.3（LICENSE）

```
Copyright (c) 2016-2021 isis agora lovecruft. All rights reserved.
Copyright (c) 2016-2021 Henry de Valence. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

1. Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright
notice, this list of conditions and the following disclaimer in the
documentation and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
contributors may be used to endorse or promote products derived from
this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS
IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED
TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A
PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE. 

========================================================================

Portions of curve25519-dalek were originally derived from Adam Langley's
Go ed25519 implementation, found at <https://github.com/agl/ed25519/>,
under the following licence:

========================================================================

Copyright (c) 2012 The Go Authors. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

   * Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.
   * Redistributions in binary form must reproduce the above
copyright notice, this list of conditions and the following disclaimer
in the documentation and/or other materials provided with the
distribution.
   * Neither the name of Google Inc. nor the names of its
contributors may be used to endorse or promote products derived from
this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS
IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED
TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A
PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER
OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

### ed25519-dalek 2.2.0（LICENSE）

```
Copyright (c) 2017-2019 isis agora lovecruft. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

1. Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright
notice, this list of conditions and the following disclaimer in the
documentation and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
contributors may be used to endorse or promote products derived from
this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS
IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED
TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A
PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

### error-code 3.4.0（LICENSE）、windows-win 3.0.0（LICENSE）

```
Boost Software License - Version 1.0 - August 17th, 2003

Permission is hereby granted, free of charge, to any person or organization
obtaining a copy of the software and accompanying documentation covered by
this license (the "Software") to use, reproduce, display, distribute,
execute, and transmit the Software, and to prepare derivative works of the
Software, and to permit third-parties to whom the Software is furnished to
do so, all subject to the following:

The copyright notices in the Software and this entire statement, including
the above license grant, this restriction and the following disclaimer,
must be included in all copies of the Software, in whole or in part, and
all derivative works of the Software, unless such copies or derivative
works are solely in the form of machine-executable object code generated by
a source language processor.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE, TITLE AND NON-INFRINGEMENT. IN NO EVENT
SHALL THE COPYRIGHT HOLDERS OR ANYONE DISTRIBUTING THE SOFTWARE BE LIABLE
FOR ANY DAMAGES OR OTHER LIABILITY, WHETHER IN CONTRACT, TORT OR OTHERWISE,
ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
DEALINGS IN THE SOFTWARE.
```

### foldhash 0.1.5（LICENSE）、foldhash 0.2.0（LICENSE）

```
Copyright (c) 2024 Orson Peters

This software is provided 'as-is', without any express or implied warranty. In
no event will the authors be held liable for any damages arising from the use of
this software.

Permission is granted to anyone to use this software for any purpose, including
commercial applications, and to alter it and redistribute it freely, subject to
the following restrictions:

1. The origin of this software must not be misrepresented; you must not claim
    that you wrote the original software. If you use this software in a product,
    an acknowledgment in the product documentation would be appreciated but is
    not required.

2. Altered source versions must be plainly marked as such, and must not be
    misrepresented as being the original software.

3. This notice may not be removed or altered from any source distribution.
```

### icu_collections 2.3.0（LICENSE）、icu_locale_core 2.3.0（LICENSE）、icu_normalizer 2.3.0（LICENSE）、icu_normalizer_data 2.3.0（LICENSE）、icu_properties 2.3.0（LICENSE）、icu_properties_data 2.3.0（LICENSE）、icu_provider 2.3.1（LICENSE）、litemap 0.8.3（LICENSE）、potential_utf 0.1.6（LICENSE）、tinystr 0.8.4（LICENSE）、writeable 0.6.4（LICENSE）、yoke 0.8.3（LICENSE）、yoke-derive 0.8.3（LICENSE）、zerofrom 0.1.8（LICENSE）、zerofrom-derive 0.1.8（LICENSE）、zerotrie 0.2.5（LICENSE）、zerovec 0.11.8（LICENSE）、zerovec-derive 0.11.6（LICENSE）

```
UNICODE LICENSE V3

COPYRIGHT AND PERMISSION NOTICE

Copyright © 2020-2024 Unicode, Inc.

NOTICE TO USER: Carefully read the following legal agreement. BY
DOWNLOADING, INSTALLING, COPYING OR OTHERWISE USING DATA FILES, AND/OR
SOFTWARE, YOU UNEQUIVOCALLY ACCEPT, AND AGREE TO BE BOUND BY, ALL OF THE
TERMS AND CONDITIONS OF THIS AGREEMENT. IF YOU DO NOT AGREE, DO NOT
DOWNLOAD, INSTALL, COPY, DISTRIBUTE OR USE THE DATA FILES OR SOFTWARE.

Permission is hereby granted, free of charge, to any person obtaining a
copy of data files and any associated documentation (the "Data Files") or
software and any associated documentation (the "Software") to deal in the
Data Files or Software without restriction, including without limitation
the rights to use, copy, modify, merge, publish, distribute, and/or sell
copies of the Data Files or Software, and to permit persons to whom the
Data Files or Software are furnished to do so, provided that either (a)
this copyright and permission notice appear with all copies of the Data
Files or Software, or (b) this copyright and permission notice appear in
associated Documentation.

THE DATA FILES AND SOFTWARE ARE PROVIDED "AS IS", WITHOUT WARRANTY OF ANY
KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF
THIRD PARTY RIGHTS.

IN NO EVENT SHALL THE COPYRIGHT HOLDER OR HOLDERS INCLUDED IN THIS NOTICE
BE LIABLE FOR ANY CLAIM, OR ANY SPECIAL INDIRECT OR CONSEQUENTIAL DAMAGES,
OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS,
WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION,
ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THE DATA
FILES OR SOFTWARE.

Except as contained in this notice, the name of a copyright holder shall
not be used in advertising or otherwise to promote the sale, use or other
dealings in these Data Files or Software without prior written
authorization of the copyright holder.

SPDX-License-Identifier: Unicode-3.0

—

Portions of ICU4X may have been adapted from ICU4C and/or ICU4J.
ICU 1.8.1 to ICU 57.1 © 1995-2016 International Business Machines Corporation and others.
```

### matchit 0.8.4（LICENSE）

```
MIT License

Copyright (c) 2022 Ibraheem Ahmed

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### matchit 0.8.4（LICENSE.httprouter）

```
BSD 3-Clause License

Copyright (c) 2013, Julien Schmidt
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
   contributors may be used to endorse or promote products derived from
   this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

### option-ext 0.2.0（LICENSE.txt）

```
Mozilla Public License Version 2.0
==================================

1. Definitions
--------------

1.1. "Contributor"
    means each individual or legal entity that creates, contributes to
    the creation of, or owns Covered Software.

1.2. "Contributor Version"
    means the combination of the Contributions of others (if any) used
    by a Contributor and that particular Contributor's Contribution.

1.3. "Contribution"
    means Covered Software of a particular Contributor.

1.4. "Covered Software"
    means Source Code Form to which the initial Contributor has attached
    the notice in Exhibit A, the Executable Form of such Source Code
    Form, and Modifications of such Source Code Form, in each case
    including portions thereof.

1.5. "Incompatible With Secondary Licenses"
    means

    (a) that the initial Contributor has attached the notice described
        in Exhibit B to the Covered Software; or

    (b) that the Covered Software was made available under the terms of
        version 1.1 or earlier of the License, but not also under the
        terms of a Secondary License.

1.6. "Executable Form"
    means any form of the work other than Source Code Form.

1.7. "Larger Work"
    means a work that combines Covered Software with other material, in 
    a separate file or files, that is not Covered Software.

1.8. "License"
    means this document.

1.9. "Licensable"
    means having the right to grant, to the maximum extent possible,
    whether at the time of the initial grant or subsequently, any and
    all of the rights conveyed by this License.

1.10. "Modifications"
    means any of the following:

    (a) any file in Source Code Form that results from an addition to,
        deletion from, or modification of the contents of Covered
        Software; or

    (b) any new file in Source Code Form that contains any Covered
        Software.

1.11. "Patent Claims" of a Contributor
    means any patent claim(s), including without limitation, method,
    process, and apparatus claims, in any patent Licensable by such
    Contributor that would be infringed, but for the grant of the
    License, by the making, using, selling, offering for sale, having
    made, import, or transfer of either its Contributions or its
    Contributor Version.

1.12. "Secondary License"
    means either the GNU General Public License, Version 2.0, the GNU
    Lesser General Public License, Version 2.1, the GNU Affero General
    Public License, Version 3.0, or any later versions of those
    licenses.

1.13. "Source Code Form"
    means the form of the work preferred for making modifications.

1.14. "You" (or "Your")
    means an individual or a legal entity exercising rights under this
    License. For legal entities, "You" includes any entity that
    controls, is controlled by, or is under common control with You. For
    purposes of this definition, "control" means (a) the power, direct
    or indirect, to cause the direction or management of such entity,
    whether by contract or otherwise, or (b) ownership of more than
    fifty percent (50%) of the outstanding shares or beneficial
    ownership of such entity.

2. License Grants and Conditions
--------------------------------

2.1. Grants

Each Contributor hereby grants You a world-wide, royalty-free,
non-exclusive license:

(a) under intellectual property rights (other than patent or trademark)
    Licensable by such Contributor to use, reproduce, make available,
    modify, display, perform, distribute, and otherwise exploit its
    Contributions, either on an unmodified basis, with Modifications, or
    as part of a Larger Work; and

(b) under Patent Claims of such Contributor to make, use, sell, offer
    for sale, have made, import, and otherwise transfer either its
    Contributions or its Contributor Version.

2.2. Effective Date

The licenses granted in Section 2.1 with respect to any Contribution
become effective for each Contribution on the date the Contributor first
distributes such Contribution.

2.3. Limitations on Grant Scope

The licenses granted in this Section 2 are the only rights granted under
this License. No additional rights or licenses will be implied from the
distribution or licensing of Covered Software under this License.
Notwithstanding Section 2.1(b) above, no patent license is granted by a
Contributor:

(a) for any code that a Contributor has removed from Covered Software;
    or

(b) for infringements caused by: (i) Your and any other third party's
    modifications of Covered Software, or (ii) the combination of its
    Contributions with other software (except as part of its Contributor
    Version); or

(c) under Patent Claims infringed by Covered Software in the absence of
    its Contributions.

This License does not grant any rights in the trademarks, service marks,
or logos of any Contributor (except as may be necessary to comply with
the notice requirements in Section 3.4).

2.4. Subsequent Licenses

No Contributor makes additional grants as a result of Your choice to
distribute the Covered Software under a subsequent version of this
License (see Section 10.2) or under the terms of a Secondary License (if
permitted under the terms of Section 3.3).

2.5. Representation

Each Contributor represents that the Contributor believes its
Contributions are its original creation(s) or it has sufficient rights
to grant the rights to its Contributions conveyed by this License.

2.6. Fair Use

This License is not intended to limit any rights You have under
applicable copyright doctrines of fair use, fair dealing, or other
equivalents.

2.7. Conditions

Sections 3.1, 3.2, 3.3, and 3.4 are conditions of the licenses granted
in Section 2.1.

3. Responsibilities
-------------------

3.1. Distribution of Source Form

All distribution of Covered Software in Source Code Form, including any
Modifications that You create or to which You contribute, must be under
the terms of this License. You must inform recipients that the Source
Code Form of the Covered Software is governed by the terms of this
License, and how they can obtain a copy of this License. You may not
attempt to alter or restrict the recipients' rights in the Source Code
Form.

3.2. Distribution of Executable Form

If You distribute Covered Software in Executable Form then:

(a) such Covered Software must also be made available in Source Code
    Form, as described in Section 3.1, and You must inform recipients of
    the Executable Form how they can obtain a copy of such Source Code
    Form by reasonable means in a timely manner, at a charge no more
    than the cost of distribution to the recipient; and

(b) You may distribute such Executable Form under the terms of this
    License, or sublicense it under different terms, provided that the
    license for the Executable Form does not attempt to limit or alter
    the recipients' rights in the Source Code Form under this License.

3.3. Distribution of a Larger Work

You may create and distribute a Larger Work under terms of Your choice,
provided that You also comply with the requirements of this License for
the Covered Software. If the Larger Work is a combination of Covered
Software with a work governed by one or more Secondary Licenses, and the
Covered Software is not Incompatible With Secondary Licenses, this
License permits You to additionally distribute such Covered Software
under the terms of such Secondary License(s), so that the recipient of
the Larger Work may, at their option, further distribute the Covered
Software under the terms of either this License or such Secondary
License(s).

3.4. Notices

You may not remove or alter the substance of any license notices
(including copyright notices, patent notices, disclaimers of warranty,
or limitations of liability) contained within the Source Code Form of
the Covered Software, except that You may alter any license notices to
the extent required to remedy known factual inaccuracies.

3.5. Application of Additional Terms

You may choose to offer, and to charge a fee for, warranty, support,
indemnity or liability obligations to one or more recipients of Covered
Software. However, You may do so only on Your own behalf, and not on
behalf of any Contributor. You must make it absolutely clear that any
such warranty, support, indemnity, or liability obligation is offered by
You alone, and You hereby agree to indemnify every Contributor for any
liability incurred by such Contributor as a result of warranty, support,
indemnity or liability terms You offer. You may include additional
disclaimers of warranty and limitations of liability specific to any
jurisdiction.

4. Inability to Comply Due to Statute or Regulation
---------------------------------------------------

If it is impossible for You to comply with any of the terms of this
License with respect to some or all of the Covered Software due to
statute, judicial order, or regulation then You must: (a) comply with
the terms of this License to the maximum extent possible; and (b)
describe the limitations and the code they affect. Such description must
be placed in a text file included with all distributions of the Covered
Software under this License. Except to the extent prohibited by statute
or regulation, such description must be sufficiently detailed for a
recipient of ordinary skill to be able to understand it.

5. Termination
--------------

5.1. The rights granted under this License will terminate automatically
if You fail to comply with any of its terms. However, if You become
compliant, then the rights granted under this License from a particular
Contributor are reinstated (a) provisionally, unless and until such
Contributor explicitly and finally terminates Your grants, and (b) on an
ongoing basis, if such Contributor fails to notify You of the
non-compliance by some reasonable means prior to 60 days after You have
come back into compliance. Moreover, Your grants from a particular
Contributor are reinstated on an ongoing basis if such Contributor
notifies You of the non-compliance by some reasonable means, this is the
first time You have received notice of non-compliance with this License
from such Contributor, and You become compliant prior to 30 days after
Your receipt of the notice.

5.2. If You initiate litigation against any entity by asserting a patent
infringement claim (excluding declaratory judgment actions,
counter-claims, and cross-claims) alleging that a Contributor Version
directly or indirectly infringes any patent, then the rights granted to
You by any and all Contributors for the Covered Software under Section
2.1 of this License shall terminate.

5.3. In the event of termination under Sections 5.1 or 5.2 above, all
end user license agreements (excluding distributors and resellers) which
have been validly granted by You or Your distributors under this License
prior to termination shall survive termination.

************************************************************************
*                                                                      *
*  6. Disclaimer of Warranty                                           *
*  -------------------------                                           *
*                                                                      *
*  Covered Software is provided under this License on an "as is"       *
*  basis, without warranty of any kind, either expressed, implied, or  *
*  statutory, including, without limitation, warranties that the       *
*  Covered Software is free of defects, merchantable, fit for a        *
*  particular purpose or non-infringing. The entire risk as to the     *
*  quality and performance of the Covered Software is with You.        *
*  Should any Covered Software prove defective in any respect, You     *
*  (not any Contributor) assume the cost of any necessary servicing,   *
*  repair, or correction. This disclaimer of warranty constitutes an   *
*  essential part of this License. No use of any Covered Software is   *
*  authorized under this License except under this disclaimer.         *
*                                                                      *
************************************************************************

************************************************************************
*                                                                      *
*  7. Limitation of Liability                                          *
*  --------------------------                                          *
*                                                                      *
*  Under no circumstances and under no legal theory, whether tort      *
*  (including negligence), contract, or otherwise, shall any           *
*  Contributor, or anyone who distributes Covered Software as          *
*  permitted above, be liable to You for any direct, indirect,         *
*  special, incidental, or consequential damages of any character      *
*  including, without limitation, damages for lost profits, loss of    *
*  goodwill, work stoppage, computer failure or malfunction, or any    *
*  and all other commercial damages or losses, even if such party      *
*  shall have been informed of the possibility of such damages. This   *
*  limitation of liability shall not apply to liability for death or   *
*  personal injury resulting from such party's negligence to the       *
*  extent applicable law prohibits such limitation. Some               *
*  jurisdictions do not allow the exclusion or limitation of           *
*  incidental or consequential damages, so this exclusion and          *
*  limitation may not apply to You.                                    *
*                                                                      *
************************************************************************

8. Litigation
-------------

Any litigation relating to this License may be brought only in the
courts of a jurisdiction where the defendant maintains its principal
place of business and such litigation shall be governed by laws of that
jurisdiction, without reference to its conflict-of-law provisions.
Nothing in this Section shall prevent a party's ability to bring
cross-claims or counter-claims.

9. Miscellaneous
----------------

This License represents the complete agreement concerning the subject
matter hereof. If any provision of this License is held to be
unenforceable, such provision shall be reformed only to the extent
necessary to make it enforceable. Any law or regulation which provides
that the language of a contract shall be construed against the drafter
shall not be used to construe this License against a Contributor.

10. Versions of the License
---------------------------

10.1. New Versions

Mozilla Foundation is the license steward. Except as provided in Section
10.3, no one other than the license steward has the right to modify or
publish new versions of this License. Each version will be given a
distinguishing version number.

10.2. Effect of New Versions

You may distribute the Covered Software under the terms of the version
of the License under which You originally received the Covered Software,
or under the terms of any subsequent version published by the license
steward.

10.3. Modified Versions

If you create software not governed by this License, and you want to
create a new license for such software, you may create and use a
modified version of this License if you rename the license and remove
any references to the name of the license steward (except to note that
such modified license differs from this License).

10.4. Distributing Source Code Form that is Incompatible With Secondary
Licenses

If You choose to distribute Source Code Form that is Incompatible With
Secondary Licenses under the terms of this version of the License, the
notice described in Exhibit B of this License must be attached.

Exhibit A - Source Code Form License Notice
-------------------------------------------

  This Source Code Form is subject to the terms of the Mozilla Public
  License, v. 2.0. If a copy of the MPL was not distributed with this
  file, You can obtain one at https://mozilla.org/MPL/2.0/.

If it is not possible or desirable to put the notice in a particular
file, then You may include the notice in a location (such as a LICENSE
file in a relevant directory) where a recipient would be likely to look
for such a notice.

You may add additional accurate notices of copyright ownership.

Exhibit B - "Incompatible With Secondary Licenses" Notice
---------------------------------------------------------

  This Source Code Form is "Incompatible With Secondary Licenses", as
  defined by the Mozilla Public License, v. 2.0.
```

### ring 0.17.14（LICENSE）

```
*ring* uses an "ISC" license, like BoringSSL used to use, for new code
files. See LICENSE-other-bits for the text of that license.

See LICENSE-BoringSSL for code that was sourced from BoringSSL under the
Apache 2.0 license. Some code that was sourced from BoringSSL under the ISC
license. In each case, the license info is at the top of the file.

See src/polyfill/once_cell/LICENSE-APACHE and src/polyfill/once_cell/LICENSE-MIT
for the license to code that was sourced from the once_cell project.
```

### ring 0.17.14（LICENSE-BoringSSL）

```
Apache License
                           Version 2.0, January 2004
                        http://www.apache.org/licenses/

   TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION

   1. Definitions.

      "License" shall mean the terms and conditions for use, reproduction,
      and distribution as defined by Sections 1 through 9 of this document.

      "Licensor" shall mean the copyright owner or entity authorized by
      the copyright owner that is granting the License.

      "Legal Entity" shall mean the union of the acting entity and all
      other entities that control, are controlled by, or are under common
      control with that entity. For the purposes of this definition,
      "control" means (i) the power, direct or indirect, to cause the
      direction or management of such entity, whether by contract or
      otherwise, or (ii) ownership of fifty percent (50%) or more of the
      outstanding shares, or (iii) beneficial ownership of such entity.

      "You" (or "Your") shall mean an individual or Legal Entity
      exercising permissions granted by this License.

      "Source" form shall mean the preferred form for making modifications,
      including but not limited to software source code, documentation
      source, and configuration files.

      "Object" form shall mean any form resulting from mechanical
      transformation or translation of a Source form, including but
      not limited to compiled object code, generated documentation,
      and conversions to other media types.

      "Work" shall mean the work of authorship, whether in Source or
      Object form, made available under the License, as indicated by a
      copyright notice that is included in or attached to the work
      (an example is provided in the Appendix below).

      "Derivative Works" shall mean any work, whether in Source or Object
      form, that is based on (or derived from) the Work and for which the
      editorial revisions, annotations, elaborations, or other modifications
      represent, as a whole, an original work of authorship. For the purposes
      of this License, Derivative Works shall not include works that remain
      separable from, or merely link (or bind by name) to the interfaces of,
      the Work and Derivative Works thereof.

      "Contribution" shall mean any work of authorship, including
      the original version of the Work and any modifications or additions
      to that Work or Derivative Works thereof, that is intentionally
      submitted to Licensor for inclusion in the Work by the copyright owner
      or by an individual or Legal Entity authorized to submit on behalf of
      the copyright owner. For the purposes of this definition, "submitted"
      means any form of electronic, verbal, or written communication sent
      to the Licensor or its representatives, including but not limited to
      communication on electronic mailing lists, source code control systems,
      and issue tracking systems that are managed by, or on behalf of, the
      Licensor for the purpose of discussing and improving the Work, but
      excluding communication that is conspicuously marked or otherwise
      designated in writing by the copyright owner as "Not a Contribution."

      "Contributor" shall mean Licensor and any individual or Legal Entity
      on behalf of whom a Contribution has been received by Licensor and
      subsequently incorporated within the Work.

   2. Grant of Copyright License. Subject to the terms and conditions of
      this License, each Contributor hereby grants to You a perpetual,
      worldwide, non-exclusive, no-charge, royalty-free, irrevocable
      copyright license to reproduce, prepare Derivative Works of,
      publicly display, publicly perform, sublicense, and distribute the
      Work and such Derivative Works in Source or Object form.

   3. Grant of Patent License. Subject to the terms and conditions of
      this License, each Contributor hereby grants to You a perpetual,
      worldwide, non-exclusive, no-charge, royalty-free, irrevocable
      (except as stated in this section) patent license to make, have made,
      use, offer to sell, sell, import, and otherwise transfer the Work,
      where such license applies only to those patent claims licensable
      by such Contributor that are necessarily infringed by their
      Contribution(s) alone or by combination of their Contribution(s)
      with the Work to which such Contribution(s) was submitted. If You
      institute patent litigation against any entity (including a
      cross-claim or counterclaim in a lawsuit) alleging that the Work
      or a Contribution incorporated within the Work constitutes direct
      or contributory patent infringement, then any patent licenses
      granted to You under this License for that Work shall terminate
      as of the date such litigation is filed.

   4. Redistribution. You may reproduce and distribute copies of the
      Work or Derivative Works thereof in any medium, with or without
      modifications, and in Source or Object form, provided that You
      meet the following conditions:

      (a) You must give any other recipients of the Work or
          Derivative Works a copy of this License; and

      (b) You must cause any modified files to carry prominent notices
          stating that You changed the files; and

      (c) You must retain, in the Source form of any Derivative Works
          that You distribute, all copyright, patent, trademark, and
          attribution notices from the Source form of the Work,
          excluding those notices that do not pertain to any part of
          the Derivative Works; and

      (d) If the Work includes a "NOTICE" text file as part of its
          distribution, then any Derivative Works that You distribute must
          include a readable copy of the attribution notices contained
          within such NOTICE file, excluding those notices that do not
          pertain to any part of the Derivative Works, in at least one
          of the following places: within a NOTICE text file distributed
          as part of the Derivative Works; within the Source form or
          documentation, if provided along with the Derivative Works; or,
          within a display generated by the Derivative Works, if and
          wherever such third-party notices normally appear. The contents
          of the NOTICE file are for informational purposes only and
          do not modify the License. You may add Your own attribution
          notices within Derivative Works that You distribute, alongside
          or as an addendum to the NOTICE text from the Work, provided
          that such additional attribution notices cannot be construed
          as modifying the License.

      You may add Your own copyright statement to Your modifications and
      may provide additional or different license terms and conditions
      for use, reproduction, or distribution of Your modifications, or
      for any such Derivative Works as a whole, provided Your use,
      reproduction, and distribution of the Work otherwise complies with
      the conditions stated in this License.

   5. Submission of Contributions. Unless You explicitly state otherwise,
      any Contribution intentionally submitted for inclusion in the Work
      by You to the Licensor shall be under the terms and conditions of
      this License, without any additional terms or conditions.
      Notwithstanding the above, nothing herein shall supersede or modify
      the terms of any separate license agreement you may have executed
      with Licensor regarding such Contributions.

   6. Trademarks. This License does not grant permission to use the trade
      names, trademarks, service marks, or product names of the Licensor,
      except as required for reasonable and customary use in describing the
      origin of the Work and reproducing the content of the NOTICE file.

   7. Disclaimer of Warranty. Unless required by applicable law or
      agreed to in writing, Licensor provides the Work (and each
      Contributor provides its Contributions) on an "AS IS" BASIS,
      WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or
      implied, including, without limitation, any warranties or conditions
      of TITLE, NON-INFRINGEMENT, MERCHANTABILITY, or FITNESS FOR A
      PARTICULAR PURPOSE. You are solely responsible for determining the
      appropriateness of using or redistributing the Work and assume any
      risks associated with Your exercise of permissions under this License.

   8. Limitation of Liability. In no event and under no legal theory,
      whether in tort (including negligence), contract, or otherwise,
      unless required by applicable law (such as deliberate and grossly
      negligent acts) or agreed to in writing, shall any Contributor be
      liable to You for damages, including any direct, indirect, special,
      incidental, or consequential damages of any character arising as a
      result of this License or out of the use or inability to use the
      Work (including but not limited to damages for loss of goodwill,
      work stoppage, computer failure or malfunction, or any and all
      other commercial damages or losses), even if such Contributor
      has been advised of the possibility of such damages.

   9. Accepting Warranty or Additional Liability. While redistributing
      the Work or Derivative Works thereof, You may choose to offer,
      and charge a fee for, acceptance of support, warranty, indemnity,
      or other liability obligations and/or rights consistent with this
      License. However, in accepting such obligations, You may act only
      on Your own behalf and on Your sole responsibility, not on behalf
      of any other Contributor, and only if You agree to indemnify,
      defend, and hold each Contributor harmless for any liability
      incurred by, or claims asserted against, such Contributor by reason
      of your accepting any such warranty or additional liability.

   END OF TERMS AND CONDITIONS

   APPENDIX: How to apply the Apache License to your work.

      To apply the Apache License to your work, attach the following
      boilerplate notice, with the fields enclosed by brackets "[]"
      replaced with your own identifying information. (Don't include
      the brackets!)  The text should be enclosed in the appropriate
      comment syntax for the file format. We also recommend that a
      file or class name and description of purpose be included on the
      same "printed page" as the copyright notice for easier
      identification within third-party archives.

   Copyright [yyyy] [name of copyright owner]

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.


Licenses for support code
-------------------------

Parts of the TLS test suite are under the Go license. This code is not included
in BoringSSL (i.e. libcrypto and libssl) when compiled, however, so
distributing code linked against BoringSSL does not trigger this license:

Copyright (c) 2009 The Go Authors. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

   * Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.
   * Redistributions in binary form must reproduce the above
copyright notice, this list of conditions and the following disclaimer
in the documentation and/or other materials provided with the
distribution.
   * Neither the name of Google Inc. nor the names of its
contributors may be used to endorse or promote products derived from
this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.


BoringSSL uses the Chromium test infrastructure to run a continuous build,
trybots etc. The scripts which manage this, and the script for generating build
metadata, are under the Chromium license. Distributing code linked against
BoringSSL does not trigger this license.

Copyright 2015 The Chromium Authors. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

   * Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.
   * Redistributions in binary form must reproduce the above
copyright notice, this list of conditions and the following disclaimer
in the documentation and/or other materials provided with the
distribution.
   * Neither the name of Google Inc. nor the names of its
contributors may be used to endorse or promote products derived from
this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

### ring 0.17.14（LICENSE-other-bits）

```
Copyright 2015-2025 Brian Smith.

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
```

### rustls-webpki 0.103.15（LICENSE）

```
Except as otherwise noted, this project is licensed under the following
(ISC-style) terms:

Copyright 2015 Brian Smith.

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHORS DISCLAIM ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

The files under third-party/chromium are licensed as described in
third-party/chromium/LICENSE.
```

### selectors 0.36.1

```
这些包没有附许可证文件，许可证是 MPL-2.0，标准文本见 https://spdx.org/licenses/MPL-2.0.html ，版权归各自的作者（见上表）。
```

### subtle 2.6.1（LICENSE）

```
Copyright (c) 2016-2017 Isis Agora Lovecruft, Henry de Valence. All rights reserved.
Copyright (c) 2016-2024 Isis Agora Lovecruft. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

1. Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright
notice, this list of conditions and the following disclaimer in the
documentation and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
contributors may be used to endorse or promote products derived from
this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS
IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED
TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A
PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

### unicode-ident 1.0.26（LICENSE-UNICODE）

```
UNICODE LICENSE V3

COPYRIGHT AND PERMISSION NOTICE

Copyright © 1991-2023 Unicode, Inc.

NOTICE TO USER: Carefully read the following legal agreement. BY
DOWNLOADING, INSTALLING, COPYING OR OTHERWISE USING DATA FILES, AND/OR
SOFTWARE, YOU UNEQUIVOCALLY ACCEPT, AND AGREE TO BE BOUND BY, ALL OF THE
TERMS AND CONDITIONS OF THIS AGREEMENT. IF YOU DO NOT AGREE, DO NOT
DOWNLOAD, INSTALL, COPY, DISTRIBUTE OR USE THE DATA FILES OR SOFTWARE.

Permission is hereby granted, free of charge, to any person obtaining a
copy of data files and any associated documentation (the "Data Files") or
software and any associated documentation (the "Software") to deal in the
Data Files or Software without restriction, including without limitation
the rights to use, copy, modify, merge, publish, distribute, and/or sell
copies of the Data Files or Software, and to permit persons to whom the
Data Files or Software are furnished to do so, provided that either (a)
this copyright and permission notice appear with all copies of the Data
Files or Software, or (b) this copyright and permission notice appear in
associated Documentation.

THE DATA FILES AND SOFTWARE ARE PROVIDED "AS IS", WITHOUT WARRANTY OF ANY
KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF
THIRD PARTY RIGHTS.

IN NO EVENT SHALL THE COPYRIGHT HOLDER OR HOLDERS INCLUDED IN THIS NOTICE
BE LIABLE FOR ANY CLAIM, OR ANY SPECIAL INDIRECT OR CONSEQUENTIAL DAMAGES,
OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS,
WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION,
ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THE DATA
FILES OR SOFTWARE.

Except as contained in this notice, the name of a copyright holder shall
not be used in advertising or otherwise to promote the sale, use or other
dealings in these Data Files or Software without prior written
authorization of the copyright holder.
```

### untrusted 0.9.0（LICENSE.txt）

```
// Copyright 2015-2016 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHORS DISCLAIM ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
```

### webpki-roots 1.0.9（LICENSE）

```
# Community Data License Agreement - Permissive - Version 2.0

This is the Community Data License Agreement - Permissive, Version
2.0 (the "agreement"). Data Provider(s) and Data Recipient(s) agree
as follows:

## 1. Provision of the Data

1.1. A Data Recipient may use, modify, and share the Data made
available by Data Provider(s) under this agreement if that Data
Recipient follows the terms of this agreement.

1.2. This agreement does not impose any restriction on a Data
Recipient's use, modification, or sharing of any portions of the
Data that are in the public domain or that may be used, modified,
or shared under any other legal exception or limitation.

## 2. Conditions for Sharing Data

2.1. A Data Recipient may share Data, with or without modifications, so
long as the Data Recipient makes available the text of this agreement
with the shared Data.

## 3. No Restrictions on Results

3.1. This agreement does not impose any restriction or obligations
with respect to the use, modification, or sharing of Results.

## 4. No Warranty; Limitation of Liability

4.1. All Data Recipients receive the Data subject to the following
terms:

THE DATA IS PROVIDED ON AN "AS IS" BASIS, WITHOUT REPRESENTATIONS,
WARRANTIES OR CONDITIONS OF ANY KIND, EITHER EXPRESS OR IMPLIED
INCLUDING, WITHOUT LIMITATION, ANY WARRANTIES OR CONDITIONS OF TITLE,
NON-INFRINGEMENT, MERCHANTABILITY OR FITNESS FOR A PARTICULAR PURPOSE.

NO DATA PROVIDER SHALL HAVE ANY LIABILITY FOR ANY DIRECT, INDIRECT,
INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING
WITHOUT LIMITATION LOST PROFITS), HOWEVER CAUSED AND ON ANY THEORY OF
LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE DATA OR RESULTS,
EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGES.

## 5. Definitions

5.1. "Data" means the material received by a Data Recipient under
this agreement.

5.2. "Data Provider" means any person who is the source of Data
provided under this agreement and in reliance on a Data Recipient's
agreement to its terms.

5.3. "Data Recipient" means any person who receives Data directly
or indirectly from a Data Provider and agrees to the terms of this
agreement.

5.4. "Results" means any outcome obtained by computational analysis
of Data, including for example machine learning models and models'
insights.
```

### x25519-dalek 2.0.1（LICENSE）

```
Copyright (c) 2017-2021 isis agora lovecruft. All rights reserved.
Copyright (c) 2019-2021 DebugSteven. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

1. Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright
notice, this list of conditions and the following disclaimer in the
documentation and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
contributors may be used to endorse or promote products derived from
this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS
IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED
TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A
PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

### zlib-rs 0.6.8（LICENSE）

```
(C) 2024 Trifecta Tech Foundation 

This software is provided 'as-is', without any express or implied
warranty. In no event will the authors be held liable for any damages
arising from the use of this software.

Permission is granted to anyone to use this software for any purpose,
including commercial applications, and to alter it and redistribute it
freely, subject to the following restrictions:

1. The origin of this software must not be misrepresented; you must not
   claim that you wrote the original software. If you use this software
   in a product, an acknowledgment in the product documentation would be
   appreciated but is not required.

2. Altered source versions must be plainly marked as such, and must not be
   misrepresented as being the original software.

3. This notice may not be removed or altered from any source distribution.
```
