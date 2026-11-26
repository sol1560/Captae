# 打进手机 App 的 Rust 依赖

由 `scripts/gen-licenses.py` 生成（不要手改）：`captae-mobile`（iOS 的 xcframework、Android 的 libcaptae_mobile.so）
在 iOS（aarch64-apple-ios）和 Android（aarch64-linux-android）上真正编进去的第三方 crate，「平台」一列写用在哪边。
生成绑定的命令行（uniffi-bindgen，只在电脑上跑）不编进 App，不列。UniFFI（MPL-2.0）没有改过，源码见表里的仓库地址。
不含只在编译时、测试时用的。许可证是「A OR B」的，Captae 有 MIT 就按 MIT、没有就按 Apache-2.0 用（「按」那一列）；
按这两种用的，版权行列在表里、许可证原文在文末；其余许可证（BSD、ISC、Zlib、MPL-2.0、Unicode-3.0、CDLA-Permissive-2.0……）
把这个 crate 自带的许可证文件原样附在文末。所有 crate 都没有改过，源码在各自的仓库和 crates.io 上对应的版本里
（MPL-2.0 的几个也一样：没有改动，源码见表里的仓库地址）。

一共 299 个。许可证表达式的分布：

- MIT OR Apache-2.0：152
- MIT：49
- Apache-2.0 OR MIT：39
- Unicode-3.0：18
- MIT/Apache-2.0：8
- MPL-2.0：7
- BSD-3-Clause：5
- Unlicense OR MIT：3
- Zlib：2
- Apache-2.0 OR ISC OR MIT：2
- Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT：2
- BSD-3-Clause OR MIT OR Apache-2.0：2
- ISC：2
- Apache-2.0：2
- Apache-2.0 / MIT：1
- Apache-2.0 AND ISC：1
- Apache-2.0 OR BSL-1.0：1
- (MIT OR Apache-2.0) AND Unicode-3.0：1
- CDLA-Permissive-2.0：1
- BSD-2-Clause OR Apache-2.0 OR MIT：1

| crate | 版本 | 平台 | 许可证 | 按 | 来源 | 版权 |
|---|---|---|---|---|---|---|
| aead | 0.5.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/traits | Copyright (c) 2019 The RustCrypto Project Developers；Copyright (c) 2019 MobileCoin, LLC |
| aes | 0.8.4 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-ciphers | Copyright (c) 2018 Artyom Pavlov |
| aes-gcm | 0.10.3 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/AEADs | Copyright (c) 2019 The RustCrypto Project Developers |
| aes-kw | 0.2.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/key-wraps/tree/aes-kw | Copyright (c) 2021-2022 RustCrypto Developers |
| aho-corasick | 1.1.5 | iOS + Android | Unlicense OR MIT | MIT | https://github.com/BurntSushi/aho-corasick | Copyright (c) 2015 Andrew Gallant |
| anyhow | 1.0.104 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/anyhow | 作者：David Tolnay |
| argon2 | 0.5.3 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/password-hashes/tree/master/argon2 | Copyright (c) 2021-2024 The RustCrypto Project Developers |
| atomic-waker | 1.1.2 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/smol-rs/atomic-waker | Copyright (c) 2016 Alex Crichton；Copyright (c) 2017 The Tokio Authors |
| base16ct | 0.2.0 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats/tree/master/base16ct | Copyright (c) 2014 Steve "Sc00bz" Thomas (steve at tobtu dot com)；Copyright (c) 2022 The RustCrypto Project Developers |
| base64 | 0.22.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/marshallpierce/rust-base64 | Copyright (c) 2015 Alice Maz |
| base64 | 0.23.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/marshallpierce/rust-base64 | Copyright (c) 2025 Alice Maz, Marshall Pierce |
| base64ct | 1.8.3 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats | Copyright (c) 2014 Steve "Sc00bz" Thomas (steve at tobtu dot com)；Copyright (c) 2021-2025 The RustCrypto Project Developers |
| bitfields | 1.0.3 | iOS + Android | MIT | MIT | https://github.com/gregorygaines/bitfields-rs | 作者：Gregory Gaines |
| bitfields-impl | 1.0.3 | iOS + Android | MIT | MIT | https://github.com/gregorygaines/bitfields-rs | 作者：Gregory Gaines |
| bitflags | 2.13.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/bitflags/bitflags | Copyright (c) 2014 The Rust Project Developers |
| bitvec | 1.1.1 | iOS + Android | MIT | MIT | https://github.com/bitvecto-rs/bitvec | Copyright (c) 2018 myrrlyn (Alexander Payne) |
| blake2 | 0.10.6 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/hashes | Copyright (c) 2015-2016 The blake2-rfc Developers, Cesar Barros；Copyright (c) 2017 Artyom Pavlov |
| block-buffer | 0.10.4 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/utils | Copyright (c) 2018-2019 The RustCrypto Project Developers |
| block-padding | 0.3.3 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/utils | Copyright (c) 2018-2019 The RustCrypto Project Developers |
| blowfish | 0.9.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-ciphers | Copyright (c) 2006-2009 Graydon Hoare；Copyright (c) 2009-2013 Mozilla Foundation |
| buffer-redux | 1.1.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/dignifiedquire/buffer-redux | Copyright 2016-2018 Austin Bonander and The Rust Project Developers；Copyright (c) 2016-2022 Austin Bonander and The Rust Project Developers |
| bumpalo | 3.20.3 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/fitzgen/bumpalo | Copyright (c) 2019 Nick Fitzgerald |
| byteorder | 1.5.0 | iOS + Android | Unlicense OR MIT | MIT | https://github.com/BurntSushi/byteorder | Copyright (c) 2015 Andrew Gallant |
| bytes | 1.12.1 | iOS + Android | MIT | MIT | https://github.com/tokio-rs/bytes | Copyright (c) 2018 Carl Lerche |
| camellia | 0.1.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-ciphers | Copyright (c) 2021 Shun Sakai |
| camino | 1.2.6 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/camino-rs/camino | 作者：Without Boats、Ashley Williams、Steve Klabnik、Rain |
| captae-remote | 0.2.0 | iOS + Android | MIT | MIT | https://crates.io/crates/captae-remote | — |
| cargo-platform | 0.3.3 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/cargo | — |
| cargo_metadata | 0.23.1 | iOS + Android | MIT | MIT | https://github.com/oli-obk/cargo_metadata | 作者：Oliver Schneider |
| cast5 | 0.11.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-ciphers | Copyright (c) 2018-2019 Friedel Ziegelmayer；Copyright (c) 2018-2019 The RustCrypto Project Developers |
| cbc | 0.1.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-modes | Copyright (c) 2018-2022 RustCrypto Developers；Copyright (c) 2018 Artyom Pavlov |
| cfb-mode | 0.8.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-modes | Copyright (c) 2018-2022 RustCrypto Developers；Copyright (c) 2018 Artyom Pavlov |
| cfg-if | 1.0.5 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/cfg-if | Copyright (c) 2014 Alex Crichton |
| cipher | 0.4.4 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/traits | Copyright (c) 2016-2020 RustCrypto Developers |
| cmac | 0.7.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/MACs | Copyright (c) 2017 Artyom Pavlov |
| combine | 4.6.8 | Android | MIT | MIT | https://github.com/Marwes/combine | Copyright (c) 2015 Markus Westerlind |
| const-oid | 0.9.6 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats/tree/master/const-oid | Copyright (c) 2020-2022 The RustCrypto Project Developers |
| convert_case | 0.10.0 | iOS + Android | MIT | MIT | https://github.com/rutrum/convert-case | Copyright (c) 2025 rutrum |
| core-foundation | 0.10.1 | iOS | MIT OR Apache-2.0 | MIT | https://github.com/servo/core-foundation-rs | Copyright (c) 2012-2013 Mozilla Foundation |
| core-foundation-sys | 0.8.7 | iOS | MIT OR Apache-2.0 | MIT | https://github.com/servo/core-foundation-rs | Copyright (c) 2012-2013 Mozilla Foundation |
| cpufeatures | 0.2.17 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/utils | Copyright (c) 2020-2025 The RustCrypto Project Developers |
| crc24 | 0.1.6 | iOS + Android | MIT/Apache-2.0 | MIT | https://github.com/sellibitze/crc24-rs.git | 作者：Sebastian Gesemann |
| crc32fast | 1.5.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/srijs/rust-crc32fast | Copyright (c) 2018 Sam Rijs, Alex Crichton and contributors |
| crypto-bigint | 0.5.5 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/crypto-bigint | Copyright (c) 2021 The RustCrypto Project Developers |
| crypto-common | 0.1.7 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/traits | Copyright (c) 2021 RustCrypto Developers |
| ctr | 0.9.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-modes | Copyright (c) 2018-2022 RustCrypto Developers；Copyright (c) 2018 Artyom Pavlov |
| curve25519-dalek | 4.1.3 | iOS + Android | BSD-3-Clause | BSD-3-Clause | https://github.com/dalek-cryptography/curve25519-dalek/tree/main/curve25519-dalek | Copyright (c) 2016-2021 isis agora lovecruft. All rights reserved.；Copyright (c) 2016-2021 Henry de Valence. All rights reserved.；Copyright (c) 2012 The Go Authors. All rights reserved. |
| cx448 | 0.1.1 | iOS + Android | BSD-3-Clause | BSD-3-Clause | https://github.com/dignifiedquire/cx448 | — |
| darling | 0.20.11 | iOS + Android | MIT | MIT | https://github.com/TedDriggs/darling | Copyright (c) 2017 Ted Driggs |
| darling_core | 0.20.11 | iOS + Android | MIT | MIT | https://github.com/TedDriggs/darling | Copyright (c) 2017 Ted Driggs |
| darling_macro | 0.20.11 | iOS + Android | MIT | MIT | https://github.com/TedDriggs/darling | Copyright (c) 2017 Ted Driggs |
| data-encoding | 2.11.1 | iOS + Android | MIT | MIT | https://github.com/ia0/data-encoding | Copyright (c) 2015-2020 Julien Cretin；Copyright (c) 2017-2020 Google Inc. |
| dbl | 0.3.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/utils | Copyright (c) 2018-2019 The RustCrypto Project Developers |
| der | 0.7.10 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats/tree/master/der | Copyright (c) 2020-2023 The RustCrypto Project Developers |
| deranged | 0.5.8 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/jhpratt/deranged | Copyright 2024 Jacob Pratt et al.；Copyright (c) 2024 Jacob Pratt et al. |
| derive_builder | 0.20.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/colin-kiegel/rust-derive-builder | Copyright (c) 2016 rust-derive-builder contributors |
| derive_builder_core | 0.20.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/colin-kiegel/rust-derive-builder | Copyright (c) 2016 rust-derive-builder contributors |
| derive_builder_macro | 0.20.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/colin-kiegel/rust-derive-builder | Copyright (c) 2016 rust-derive-builder contributors |
| derive_more | 2.1.1 | iOS + Android | MIT | MIT | https://github.com/JelteF/derive_more | Copyright (c) 2016 Jelte Fennema |
| derive_more-impl | 2.1.1 | iOS + Android | MIT | MIT | https://github.com/JelteF/derive_more | Copyright (c) 2016 Jelte Fennema |
| des | 0.8.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-ciphers | Copyright 2017 Gulshan Singh；Copyright (c) 2017 Gulshan Singh, Antoni Boucher, Artyom Pavlov |
| digest | 0.10.7 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/traits | Copyright (c) 2017 Artyom Pavlov |
| directories | 6.0.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/soc/directories-rs | Copyright (c) 2018 directories-rs contributors |
| dirs-sys | 0.5.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/dirs-dev/dirs-sys-rs | Copyright (c) 2018-2019 dirs-rs contributors |
| displaydoc | 0.2.7 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/yaahc/displaydoc | 作者：Jane Lusby |
| dsa | 0.6.3 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/signatures/tree/master/dsa | Copyright 2018-2022 RustCrypto Developers；Copyright (c) 2018-2023 RustCrypto Developers |
| eax | 0.5.0 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/AEADs | Copyright (c) 2020 The RustCrypto Project Developers |
| ecdsa | 0.16.9 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/signatures/tree/master/ecdsa | Copyright 2018-2022 RustCrypto Developers；Copyright (c) 2018-2022 RustCrypto Developers |
| ed25519 | 2.2.3 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/signatures/tree/master/ed25519 | Copyright 2018-2022 RustCrypto Developers；Copyright (c) 2018-2023 RustCrypto Developers |
| ed25519-dalek | 2.2.0 | iOS + Android | BSD-3-Clause | BSD-3-Clause | https://github.com/dalek-cryptography/curve25519-dalek/tree/main/ed25519-dalek | Copyright (c) 2017-2019 isis agora lovecruft. All rights reserved. |
| elliptic-curve | 0.13.8 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/traits/tree/master/elliptic-curve | Copyright (c) 2020-2022 RustCrypto Developers |
| equivalent | 1.0.2 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/indexmap-rs/equivalent | Copyright (c) 2016--2023 |
| errno | 0.3.14 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/lambda-fairy/rust-errno | Copyright (c) 2014 Chris Wong |
| fallible-iterator | 0.3.0 | iOS + Android | MIT/Apache-2.0 | MIT | https://github.com/sfackler/rust-fallible-iterator | Copyright (c) 2015 The rust-openssl-verify Developers |
| fallible-streaming-iterator | 0.1.9 | iOS + Android | MIT/Apache-2.0 | MIT | https://github.com/sfackler/fallible-streaming-iterator | Copyright (c) 2016 The fallible-streaming-iterator Developers |
| fastrand | 2.5.0 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/smol-rs/fastrand | 作者：Stjepan Glavina |
| ff | 0.13.1 | iOS + Android | MIT/Apache-2.0 | MIT | https://github.com/zkcrypto/ff | Copyright (c) 2017 Sean Bowe |
| flate2 | 1.1.10 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/flate2-rs | Copyright (c) 2014-2026 Alex Crichton |
| fnv | 1.0.7 | iOS + Android | Apache-2.0 / MIT | MIT | https://github.com/servo/rust-fnv | Copyright (c) 2017 Contributors |
| foldhash | 0.1.5 | iOS + Android | Zlib | Zlib | https://github.com/orlp/foldhash | Copyright (c) 2024 Orson Peters |
| form_urlencoded | 1.2.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/servo/rust-url | Copyright (c) 2013-2016 The rust-url developers |
| fs-err | 3.3.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/andrewhickman/fs-err | 作者：Andrew Hickman |
| funty | 2.0.0 | iOS + Android | MIT | MIT | https://github.com/myrrlyn/funty | Copyright (c) 2020 myrrlyn (Alexander Payne) |
| futures-channel | 0.3.34 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/futures-rs | Copyright (c) 2016 Alex Crichton；Copyright (c) 2017 The Tokio Authors |
| futures-core | 0.3.34 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/futures-rs | Copyright (c) 2016 Alex Crichton；Copyright (c) 2017 The Tokio Authors |
| futures-macro | 0.3.34 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/futures-rs | Copyright (c) 2016 Alex Crichton；Copyright (c) 2017 The Tokio Authors |
| futures-sink | 0.3.34 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/futures-rs | Copyright (c) 2016 Alex Crichton；Copyright (c) 2017 The Tokio Authors |
| futures-task | 0.3.34 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/futures-rs | Copyright (c) 2016 Alex Crichton；Copyright (c) 2017 The Tokio Authors |
| futures-util | 0.3.34 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/futures-rs | Copyright (c) 2016 Alex Crichton；Copyright (c) 2017 The Tokio Authors |
| generic-array | 0.14.7 | iOS + Android | MIT | MIT | https://github.com/fizyk20/generic-array.git | Copyright (c) 2015 Bartłomiej Kamiński |
| getrandom | 0.2.17 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-random/getrandom | Copyright (c) 2018-2024 The rust-random Project Developers；Copyright (c) 2014 The Rust Project Developers |
| getrandom | 0.3.4 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-random/getrandom | Copyright (c) 2018-2025 The rust-random Project Developers；Copyright (c) 2014 The Rust Project Developers |
| getrandom | 0.4.3 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-random/getrandom | Copyright (c) 2018-2026 The rust-random Project Developers；Copyright (c) 2014 The Rust Project Developers |
| ghash | 0.5.1 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/universal-hashes | Copyright (c) 2019 RustCrypto Developers |
| group | 0.13.0 | iOS + Android | MIT/Apache-2.0 | MIT | https://github.com/zkcrypto/group | 作者：Sean Bowe、Jack Grigg |
| hashbrown | 0.15.5 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/hashbrown | Copyright (c) 2016 Amanieu d'Antras |
| hashbrown | 0.17.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/hashbrown | Copyright (c) 2016 Amanieu d'Antras |
| hashlink | 0.10.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/kyren/hashlink | 作者：kyren |
| heck | 0.5.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/withoutboats/heck | Copyright (c) 2015 The Rust Project Developers |
| hex | 0.4.3 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/KokaKiwi/rust-hex | Copyright (c) 2013-2014 The Rust Project Developers.；Copyright (c) 2015-2020 The rust-hex Developers |
| hkdf | 0.12.4 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/KDFs/ | Copyright (c) 2015-2018 Vlad Filippov；Copyright (c) 2018-2021 RustCrypto Developers |
| hmac | 0.12.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/MACs | Copyright (c) 2017 Artyom Pavlov |
| http | 1.5.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/hyperium/http | Copyright 2017 http-rs authors；Copyright (c) 2017 http-rs authors |
| http-body | 1.1.0 | iOS + Android | MIT | MIT | https://github.com/hyperium/http-body | Copyright (c) 2019-2026 Sean McArthur & Hyper Contributors |
| http-body-util | 0.1.5 | iOS + Android | MIT | MIT | https://github.com/hyperium/http-body | Copyright (c) 2019-2026 Sean McArthur & Hyper Contributors |
| httparse | 1.10.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/seanmonstar/httparse | Copyright (c) 2015-2025 Sean McArthur |
| hyper | 1.11.1 | iOS + Android | MIT | MIT | https://github.com/hyperium/hyper | Copyright (c) 2014-2026 Sean McArthur |
| hyper-rustls | 0.27.10 | iOS + Android | Apache-2.0 OR ISC OR MIT | MIT | https://github.com/rustls/hyper-rustls | Copyright (c) 2016, Joseph Birr-Pixton <jpixton@gmail.com>；Copyright (c) 2016 Joseph Birr-Pixton <jpixton@gmail.com> |
| hyper-util | 0.1.21 | iOS + Android | MIT | MIT | https://github.com/hyperium/hyper-util | Copyright (c) 2023-2025 Sean McArthur |
| icu_collections | 2.3.0 | iOS + Android | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| icu_locale_core | 2.3.0 | iOS + Android | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| icu_normalizer | 2.3.0 | iOS + Android | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| icu_normalizer_data | 2.3.0 | iOS + Android | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| icu_properties | 2.3.0 | iOS + Android | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| icu_properties_data | 2.3.0 | iOS + Android | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| icu_provider | 2.3.1 | iOS + Android | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| idea | 0.5.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-ciphers | Copyright (c) 2017 Damian Czaja |
| ident_case | 1.0.1 | iOS + Android | MIT/Apache-2.0 | MIT | https://github.com/TedDriggs/ident_case | 作者：Ted Driggs |
| idna | 1.1.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/servo/rust-url/ | Copyright (c) 2013-2025 The rust-url developers |
| idna_adapter | 1.2.2 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/hsivonen/idna_adapter | Copyright (c) The rust-url developers |
| indexmap | 2.14.2 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/indexmap-rs/indexmap | Copyright (c) 2016--2017 |
| inout | 0.1.4 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/utils | Copyright (c) 2022 The RustCrypto Project Developers；Copyright (c) 2022 Artyom Pavlov |
| ipnet | 2.12.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/krisprice/ipnet | Copyright 2017 Juniper Networks, Inc. |
| itoa | 1.0.18 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/itoa | 作者：David Tolnay |
| jni | 0.22.4 | Android | MIT OR Apache-2.0 | MIT | https://github.com/jni-rs/jni-rs | 作者：jni team |
| jni-macros | 0.22.4 | Android | MIT OR Apache-2.0 | MIT | https://github.com/jni-rs/jni-rs | — |
| jni-sys | 0.4.1 | Android | MIT OR Apache-2.0 | MIT | https://github.com/jni-rs/jni-sys | Copyright (c) 2015 The rust-jni-sys Developers |
| jni-sys-macros | 0.4.1 | Android | MIT OR Apache-2.0 | MIT | https://github.com/jni-rs/jni-sys | 作者：Robert Bragg |
| k256 | 0.13.4 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/elliptic-curves/tree/master/k256 | Copyright (c) 2020-2024 RustCrypto Developers |
| keccak | 0.1.6 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/sponges/tree/master/keccak | Copyright (c) 2018-2022 RustCrypto Developers |
| lazy_static | 1.5.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang-nursery/lazy-static.rs | Copyright (c) 2010 The Rust Project Developers |
| libc | 0.2.189 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/libc | Copyright (c) The Rust Project Developers |
| libm | 0.2.16 | iOS + Android | MIT | MIT | https://github.com/rust-lang/compiler-builtins | Copyright (c) 2018 Jorge Aparicio；copyright:；Copyright © 2005-2020 Rich Felker, et al.；Copyright © 1993,2004 Sun Microsystems or；Copyright © 2003-2011 David Schultz or；Copyright © 2003-2009 Steven G. Kargl or；Copyright © 2003-2009 Bruce D. Evans or；Copyright © 2008 Stephen L. Moshier or；Copyright © 2017-2018 Arm Limited |
| libsqlite3-sys | 0.35.0 | iOS + Android | MIT | MIT | https://github.com/rusqlite/rusqlite | Copyright (c) 2014 The rusqlite developers |
| linux-raw-sys | 0.12.1 | Android | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | MIT | https://github.com/sunfishcode/linux-raw-sys | 作者：Dan Gohman |
| litemap | 0.8.3 | iOS + Android | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| lock_api | 0.4.14 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/Amanieu/parking_lot | Copyright (c) 2016 The Rust Project Developers |
| log | 0.4.34 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/log | Copyright (c) 2014 The Rust Project Developers |
| md-5 | 0.10.6 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/hashes | Copyright (c) 2006-2009 Graydon Hoare；Copyright (c) 2009-2013 Mozilla Foundation；Copyright (c) 2016 Artyom Pavlov |
| memchr | 2.8.3 | iOS + Android | Unlicense OR MIT | MIT | https://github.com/BurntSushi/memchr | Copyright (c) 2015 Andrew Gallant |
| mio | 1.2.3 | iOS + Android | MIT | MIT | https://github.com/tokio-rs/mio | Copyright (c) 2014 Carl Lerche and other MIO contributors |
| nom | 8.0.0 | iOS + Android | MIT | MIT | https://github.com/rust-bakery/nom | Copyright (c) 2014-2019 Geoffroy Couprie |
| num-bigint-dig | 0.8.6 | iOS + Android | MIT/Apache-2.0 | MIT | https://github.com/dignifiedquire/num-bigint | Copyright (c) 2014 The Rust Project Developers |
| num-conv | 0.2.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/jhpratt/num-conv | Copyright (c) Jacob Pratt |
| num-integer | 0.1.47 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-num/num-integer | Copyright (c) 2014 The Rust Project Developers |
| num-iter | 0.1.46 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-num/num-iter | Copyright (c) 2014 The Rust Project Developers |
| num-traits | 0.2.19 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-num/num-traits | Copyright (c) 2014 The Rust Project Developers |
| num_enum | 0.7.6 | iOS + Android | BSD-3-Clause OR MIT OR Apache-2.0 | MIT | https://github.com/illicitonion/num_enum | Copyright (c) 2018, Daniel Wagner-Hall |
| num_enum_derive | 0.7.6 | iOS + Android | BSD-3-Clause OR MIT OR Apache-2.0 | MIT | https://github.com/illicitonion/num_enum | Copyright (c) 2018, Daniel Wagner-Hall |
| num_threads | 0.1.7 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/jhpratt/num_threads | Copyright 2021 Jacob Pratt；Copyright (c) 2021 Jacob Pratt |
| ocb3 | 0.1.0 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/AEADs | Copyright (c) 2023 The RustCrypto Project Developers |
| once_cell | 1.21.4 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/matklad/once_cell | 作者：Aleksey Kladov |
| opaque-debug | 0.3.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/utils | Copyright (c) 2018-2024 The RustCrypto Project Developers |
| option-ext | 0.2.0 | iOS + Android | MPL-2.0 | MPL-2.0 | https://github.com/soc/option-ext.git | 作者：Simon Ochsenreither |
| p256 | 0.13.2 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/elliptic-curves/tree/master/p256 | Copyright (c) 2020-2023 RustCrypto Developers |
| p384 | 0.13.1 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/elliptic-curves/tree/master/p384 | Copyright (c) 2020-2021 RustCrypto Developers |
| p521 | 0.13.3 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/elliptic-curves/tree/master/p521 | Copyright (c) 2020-2022 RustCrypto Developers |
| parking_lot | 0.12.5 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/Amanieu/parking_lot | Copyright (c) 2016 The Rust Project Developers |
| parking_lot_core | 0.9.12 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/Amanieu/parking_lot | Copyright (c) 2016 The Rust Project Developers |
| password-hash | 0.5.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/traits/tree/master/password-hash | Copyright (c) 2020-2023 RustCrypto Developers |
| pem-rfc7468 | 0.7.0 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats/tree/master/pem-rfc7468 | Copyright (c) 2021 The RustCrypto Project Developers |
| percent-encoding | 2.3.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/servo/rust-url/ | Copyright (c) 2013-2025 The rust-url developers |
| pgp | 0.21.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rpgp/rpgp | 作者：dignifiedquire |
| pin-project-lite | 0.2.17 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/taiki-e/pin-project-lite | — |
| pkcs1 | 0.7.5 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats/tree/master/pkcs1 | Copyright (c) 2021-2023 The RustCrypto Project Developers |
| pkcs8 | 0.10.2 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats/tree/master/pkcs8 | Copyright (c) 2020-2023 The RustCrypto Project Developers |
| polyval | 0.6.2 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/universal-hashes | Copyright (c) 2019-2023 RustCrypto Developers |
| potential_utf | 0.1.6 | iOS + Android | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| powerfmt | 0.2.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/jhpratt/powerfmt | Copyright 2023 Jacob Pratt et al.；Copyright (c) 2023 Jacob Pratt et al. |
| ppv-lite86 | 0.2.21 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/cryptocorrosion/cryptocorrosion | Copyright 2019 The CryptoCorrosion Contributors；Copyright (c) 2019 The CryptoCorrosion Contributors |
| primeorder | 0.13.6 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/elliptic-curves/tree/master/primeorder | Copyright (c) 2020-2023 RustCrypto Developers |
| proc-macro2 | 1.0.107 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/proc-macro2 | 作者：David Tolnay、Alex Crichton |
| quote | 1.0.47 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/quote | 作者：David Tolnay |
| radium | 0.7.0 | iOS + Android | MIT | MIT | https://github.com/bitvecto-rs/radium | Copyright (c) 2019 kneecaw (Nika Layzell) |
| rand | 0.8.8 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-random/rand | Copyright 2018 Developers of the Rand project；Copyright (c) 2014 The Rust Project Developers |
| rand | 0.9.5 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-random/rand | Copyright 2018 Developers of the Rand project；Copyright (c) 2014 The Rust Project Developers |
| rand_chacha | 0.3.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-random/rand | Copyright 2018 Developers of the Rand project；Copyright (c) 2014 The Rust Project Developers |
| rand_chacha | 0.9.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-random/rand | Copyright 2018 Developers of the Rand project；Copyright (c) 2014 The Rust Project Developers |
| rand_core | 0.6.4 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-random/rand | Copyright 2018 Developers of the Rand project；Copyright (c) 2014 The Rust Project Developers |
| rand_core | 0.9.5 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-random/rand | Copyright 2018 Developers of the Rand project；Copyright (c) 2014 The Rust Project Developers |
| regex | 1.13.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/regex | Copyright (c) 2014 The Rust Project Developers |
| regex-automata | 0.4.18 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/regex | Copyright (c) 2014 The Rust Project Developers |
| regex-syntax | 0.8.11 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/regex | Copyright (c) 2014 The Rust Project Developers |
| replace_with | 0.1.8 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/alecmocatta/replace_with | 作者：Alec Mocatta |
| reqwest | 0.12.28 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/seanmonstar/reqwest | Copyright 2016 Sean McArthur；Copyright (c) 2016-2025 Sean McArthur |
| rfc6979 | 0.4.0 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/signatures/tree/master/rfc6979 | Copyright 2018-2022 RustCrypto Developers；Copyright (c) 2018-2022 RustCrypto Developers |
| ring | 0.17.14 | iOS + Android | Apache-2.0 AND ISC | Apache-2.0 + ISC | https://github.com/briansmith/ring | Copyright (c) 2009 The Go Authors. All rights reserved.；Copyright 2015 The Chromium Authors. All rights reserved.；Copyright 2015-2025 Brian Smith. |
| ripemd | 0.1.3 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/hashes | Copyright (c) 2021 RustCrypto Developers |
| rsa | 0.9.10 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/RSA | 作者：RustCrypto Developers、dignifiedquire |
| rusqlite | 0.37.0 | iOS + Android | MIT | MIT | https://github.com/rusqlite/rusqlite | Copyright (c) 2014 The rusqlite developers |
| rustix | 1.1.5 | iOS + Android | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | MIT | https://github.com/bytecodealliance/rustix | 作者：Dan Gohman、Jakub Konka |
| rustls | 0.23.45 | iOS + Android | Apache-2.0 OR ISC OR MIT | MIT | https://github.com/rustls/rustls | Copyright (c) 2016, Joseph Birr-Pixton <jpixton@gmail.com>；Copyright (c) 2016 Joseph Birr-Pixton <jpixton@gmail.com> |
| rustls-pki-types | 1.15.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rustls/pki-types | Copyright 2023 Dirkjan Ochtman；Copyright (c) 2023 Dirkjan Ochtman <dirkjan@ochtman.nl> |
| rustls-platform-verifier | 0.7.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rustls/rustls-platform-verifier | Copyright (c) 2022 1Password |
| rustls-platform-verifier-android | 0.2.0 | Android | MIT OR Apache-2.0 | MIT | https://github.com/rustls/rustls-platform-verifier | — |
| rustls-webpki | 0.103.15 | iOS + Android | ISC | ISC | https://github.com/rustls/webpki | Copyright 2015 Brian Smith. |
| rustversion | 1.0.23 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/rustversion | 作者：David Tolnay |
| ryu | 1.0.23 | iOS + Android | Apache-2.0 OR BSL-1.0 | Apache-2.0 | https://github.com/dtolnay/ryu | 作者：David Tolnay |
| scopeguard | 1.2.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/bluss/scopeguard | Copyright (c) 2016-2019 Ulrik Sverdrup "bluss" and scopeguard developers |
| sec1 | 0.7.3 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats/tree/master/sec1 | Copyright (c) 2021-2022 The RustCrypto Project Developers |
| security-framework | 3.7.0 | iOS | MIT OR Apache-2.0 | MIT | https://github.com/kornelski/rust-security-framework | Copyright (c) 2015 Steven Fackler |
| security-framework-sys | 2.17.0 | iOS | MIT OR Apache-2.0 | MIT | https://github.com/kornelski/rust-security-framework | Copyright (c) 2015 Steven Fackler |
| semver | 1.0.28 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/semver | 作者：David Tolnay |
| serde | 1.0.229 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/serde-rs/serde | 作者：Erick Tryzelaar、David Tolnay |
| serde_core | 1.0.229 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/serde-rs/serde | 作者：Erick Tryzelaar、David Tolnay |
| serde_derive | 1.0.229 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/serde-rs/serde | 作者：Erick Tryzelaar、David Tolnay |
| serde_json | 1.0.151 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/serde-rs/json | 作者：Erick Tryzelaar、David Tolnay |
| serde_spanned | 1.1.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/toml-rs/toml | Copyright (c) Individual contributors |
| serde_urlencoded | 0.7.1 | iOS + Android | MIT/Apache-2.0 | MIT | https://github.com/nox/serde_urlencoded | Copyright (c) 2016 Anthony Ramine |
| serdect | 0.2.0 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats/tree/master/serdect | Copyright (c) 2022 The RustCrypto Project Developers |
| serdect | 0.3.0 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats | Copyright (c) 2022 The RustCrypto Project Developers |
| sha1 | 0.10.7 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/hashes | Copyright (c) 2006-2009 Graydon Hoare；Copyright (c) 2009-2013 Mozilla Foundation；Copyright (c) 2016 Artyom Pavlov |
| sha1-checked | 0.10.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/hashes | Copyright (c) 2024 The RustCrypto Project Developers |
| sha2 | 0.10.9 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/hashes | Copyright (c) 2006-2009 Graydon Hoare；Copyright (c) 2009-2013 Mozilla Foundation；Copyright (c) 2016 Artyom Pavlov |
| sha3 | 0.10.9 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/hashes | Copyright (c) 2006-2009 Graydon Hoare；Copyright (c) 2009-2013 Mozilla Foundation；Copyright (c) 2014 Sébastien Martini；Copyright (c) 2016-2023 Artyom Pavlov, Marek Kotewicz |
| signal-hook-registry | 1.4.8 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/vorner/signal-hook | Copyright (c) 2017 tokio-jsonrpc developers |
| signature | 2.2.0 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/traits/tree/master/signature | Copyright (c) 2018-2023 RustCrypto Developers |
| simd-adler32 | 0.3.10 | iOS + Android | MIT | MIT | https://github.com/mcountryman/simd-adler32 | Copyright (c) [2021] [Marvin Countryman] |
| simd_cesu8 | 1.2.0 | Android | Apache-2.0 OR MIT | MIT | https://github.com/seancroach/simd_cesu8 | 作者：Sean C. Roach |
| simdutf8 | 0.1.5 | Android | MIT OR Apache-2.0 | MIT | https://github.com/rusticstuff/simdutf8 | 作者：Hans Kratz |
| siphasher | 1.0.4 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/jedisct1/rust-siphash | Copyright 2012-2016 The Rust Project Developers.；Copyright 2016-2026 Frank Denis. |
| slab | 0.4.12 | iOS + Android | MIT | MIT | https://github.com/tokio-rs/slab | Copyright (c) 2019 Carl Lerche |
| smallvec | 1.16.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/servo/rust-smallvec | Copyright (c) 2018 The Servo Project Developers |
| snafu | 0.9.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/shepmaster/snafu | Copyright 2019- Jake Goulding；Copyright (c) 2019- Jake Goulding |
| snafu-derive | 0.9.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/shepmaster/snafu | Copyright 2019- Jake Goulding；Copyright (c) 2019- Jake Goulding |
| socket2 | 0.6.5 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rust-lang/socket2 | Copyright (c) 2014 Alex Crichton |
| spake2 | 0.4.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/PAKEs/tree/master/spake2 | Copyright (c) 2017-2023 Brian Warner |
| spin | 0.9.9 | iOS + Android | MIT | MIT | https://github.com/mvdnes/spin-rs.git | Copyright (c) 2014 Mathijs van de Nes |
| spki | 0.7.3 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/formats/tree/master/spki | Copyright (c) 2021-2023 The RustCrypto Project Developers |
| stable_deref_trait | 1.2.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/storyyeller/stable_deref_trait | Copyright (c) 2017 Robert Grosse |
| static_assertions | 1.1.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/nvzqz/static-assertions-rs | Copyright (c) 2017 Nikolai Vazquez |
| strsim | 0.11.1 | iOS + Android | MIT | MIT | https://github.com/rapidfuzz/strsim-rs | Copyright (c) 2015 Danny Guo；Copyright (c) 2016 Titus Wormer <tituswormer@gmail.com>；Copyright (c) 2018 Akash Kurdekar |
| subtle | 2.6.1 | iOS + Android | BSD-3-Clause | BSD-3-Clause | https://github.com/dalek-cryptography/subtle | Copyright (c) 2016-2017 Isis Agora Lovecruft, Henry de Valence. All rights reserved.；Copyright (c) 2016-2024 Isis Agora Lovecruft. All rights reserved. |
| syn | 2.0.119 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/syn | 作者：David Tolnay |
| syn | 3.0.6 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/syn | 作者：David Tolnay |
| sync_wrapper | 1.0.2 | iOS + Android | Apache-2.0 | Apache-2.0 | https://github.com/Actyx/sync_wrapper | 作者：Actyx AG |
| synstructure | 0.14.0 | iOS + Android | MIT | MIT | https://github.com/mystor/synstructure | Copyright 2016 Nika Layzell |
| sys-locale | 0.3.2 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/1Password/sys-locale | Copyright (c) 2021 1Password |
| tap | 1.0.1 | iOS + Android | MIT | MIT | https://github.com/myrrlyn/tap | Copyright (c) 2017 Elliot Linder <darfink@gmail.com> |
| tempfile | 3.27.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/Stebalien/tempfile | Copyright (c) 2015 Steven Allen |
| thiserror | 2.0.21 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/thiserror | 作者：David Tolnay |
| thiserror-impl | 2.0.21 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/dtolnay/thiserror | 作者：David Tolnay |
| time | 0.3.55 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/time-rs/time | Copyright (c) Jacob Pratt et al. |
| time-core | 0.1.9 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/time-rs/time | Copyright (c) Jacob Pratt et al. |
| time-macros | 0.2.32 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/time-rs/time | Copyright (c) Jacob Pratt et al. |
| tinystr | 0.8.4 | iOS + Android | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| tokio | 1.53.1 | iOS + Android | MIT | MIT | https://github.com/tokio-rs/tokio | Copyright (c) Tokio Contributors |
| tokio-macros | 2.7.2 | iOS + Android | MIT | MIT | https://github.com/tokio-rs/tokio | Copyright (c) 2019 Yoshua Wuyts；Copyright (c) Tokio Contributors |
| tokio-rustls | 0.26.5 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/rustls/tokio-rustls | Copyright 2017 quininer kel；Copyright (c) 2017 quininer kel |
| tokio-tungstenite | 0.29.0 | iOS + Android | MIT | MIT | https://github.com/snapview/tokio-tungstenite | Copyright (c) 2017 Daniel Abramov；Copyright (c) 2017 Alexey Galakhov |
| tokio-util | 0.7.19 | iOS + Android | MIT | MIT | https://github.com/tokio-rs/tokio | Copyright (c) Tokio Contributors |
| toml | 1.1.6+spec-1.1.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/toml-rs/toml | Copyright (c) Individual contributors |
| toml_datetime | 1.1.1+spec-1.1.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/toml-rs/toml | Copyright (c) Individual contributors |
| toml_parser | 1.1.3+spec-1.1.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/toml-rs/toml | Copyright (c) Individual contributors |
| toml_writer | 1.1.2+spec-1.1.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/toml-rs/toml | Copyright (c) Individual contributors |
| tower | 0.5.3 | iOS + Android | MIT | MIT | https://github.com/tower-rs/tower | Copyright (c) 2019 Tower Contributors |
| tower-http | 0.6.11 | iOS + Android | MIT | MIT | https://github.com/tower-rs/tower-http | Copyright (c) 2019-2021 Tower Contributors |
| tower-layer | 0.3.3 | iOS + Android | MIT | MIT | https://github.com/tower-rs/tower | Copyright (c) 2019 Tower Contributors |
| tower-service | 0.3.3 | iOS + Android | MIT | MIT | https://github.com/tower-rs/tower | Copyright (c) 2019 Tower Contributors |
| tracing | 0.1.44 | iOS + Android | MIT | MIT | https://github.com/tokio-rs/tracing | Copyright (c) 2019 Tokio Contributors |
| tracing-attributes | 0.1.31 | iOS + Android | MIT | MIT | https://github.com/tokio-rs/tracing | Copyright (c) 2019 Tokio Contributors |
| tracing-core | 0.1.36 | iOS + Android | MIT | MIT | https://github.com/tokio-rs/tracing | Copyright (c) 2019 Tokio Contributors |
| try-lock | 0.2.5 | iOS + Android | MIT | MIT | https://github.com/seanmonstar/try-lock | Copyright (c) 2018-2023 Sean McArthur；Copyright (c) 2016 Alex Crichton |
| tungstenite | 0.29.0 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/snapview/tungstenite-rs | Copyright (c) 2017 Alexey Galakhov；Copyright (c) 2016 Jason Housley |
| twofish | 0.7.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/block-ciphers | Copyright (c) 2017 Alexander Krotov |
| typenum | 1.20.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/paholg/typenum | Copyright 2014 Paho Lurie-Gregg；Copyright (c) 2014 Paho Lurie-Gregg |
| unicode-ident | 1.0.26 | iOS + Android | (MIT OR Apache-2.0) AND Unicode-3.0 | MIT + Unicode-3.0 | https://github.com/dtolnay/unicode-ident | Copyright © 1991-2023 Unicode, Inc. |
| unicode-segmentation | 1.13.3 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/unicode-rs/unicode-segmentation | Copyright (c) 2015 The Rust Project Developers |
| unicode-xid | 0.2.6 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/unicode-rs/unicode-xid | Copyright (c) 2015 The Rust Project Developers |
| uniffi | 0.32.2 | iOS + Android | MPL-2.0 | MPL-2.0 | https://github.com/mozilla/uniffi-rs | — |
| uniffi_core | 0.32.2 | iOS + Android | MPL-2.0 | MPL-2.0 | https://github.com/mozilla/uniffi-rs | — |
| uniffi_internal_macros | 0.32.2 | iOS + Android | MPL-2.0 | MPL-2.0 | https://github.com/mozilla/uniffi-rs | — |
| uniffi_macros | 0.32.2 | iOS + Android | MPL-2.0 | MPL-2.0 | https://github.com/mozilla/uniffi-rs | — |
| uniffi_meta | 0.32.2 | iOS + Android | MPL-2.0 | MPL-2.0 | https://github.com/mozilla/uniffi-rs | — |
| uniffi_pipeline | 0.32.2 | iOS + Android | MPL-2.0 | MPL-2.0 | https://github.com/mozilla/uniffi-rs | — |
| universal-hash | 0.5.1 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/RustCrypto/traits | Copyright (c) 2019-2020 RustCrypto Developers |
| untrusted | 0.9.0 | iOS + Android | ISC | ISC | https://github.com/briansmith/untrusted | Copyright 2015-2016 Brian Smith. |
| url | 2.5.8 | iOS + Android | MIT OR Apache-2.0 | MIT | https://github.com/servo/rust-url | Copyright (c) 2013-2025 The rust-url developers |
| utf8_iter | 1.0.4 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/hsivonen/utf8_iter | Copyright Mozilla Foundation |
| want | 0.3.1 | iOS + Android | MIT | MIT | https://github.com/seanmonstar/want | Copyright (c) 2018-2019 Sean McArthur |
| webpki-roots | 1.0.9 | iOS + Android | CDLA-Permissive-2.0 | CDLA-Permissive-2.0 | https://github.com/rustls/webpki-roots | — |
| winnow | 1.0.4 | iOS + Android | MIT | MIT | https://github.com/winnow-rs/winnow | — |
| writeable | 0.6.4 | iOS + Android | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| wyz | 0.5.1 | iOS + Android | MIT | MIT | https://github.com/myrrlyn/wyz | Copyright (c) 2018 myrrlyn (Alexander Payne) |
| x25519-dalek | 2.0.1 | iOS + Android | BSD-3-Clause | BSD-3-Clause | https://github.com/dalek-cryptography/curve25519-dalek/tree/main/x25519-dalek | Copyright (c) 2017-2021 isis agora lovecruft. All rights reserved.；Copyright (c) 2019-2021 DebugSteven. All rights reserved. |
| yoke | 0.8.3 | iOS + Android | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| yoke-derive | 0.8.3 | iOS + Android | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| zerocopy | 0.8.59 | iOS + Android | BSD-2-Clause OR Apache-2.0 OR MIT | MIT | https://github.com/google/zerocopy | Copyright 2023 The Fuchsia Authors；Copyright 2019 The Fuchsia Authors. |
| zerofrom | 0.1.8 | iOS + Android | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| zerofrom-derive | 0.1.8 | iOS + Android | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| zeroize | 1.9.0 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/utils | Copyright (c) 2018-2026 The RustCrypto Project Developers |
| zeroize_derive | 1.5.0 | iOS + Android | Apache-2.0 OR MIT | MIT | https://github.com/RustCrypto/utils | Copyright (c) 2019-2026 The RustCrypto Project Developers |
| zerotrie | 0.2.5 | iOS + Android | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| zerovec | 0.11.8 | iOS + Android | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| zerovec-derive | 0.11.6 | iOS + Android | Unicode-3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x | Copyright © 2020-2024 Unicode, Inc. |
| zip | 4.6.1 | iOS + Android | MIT | MIT | https://github.com/zip-rs/zip2.git | Copyright (c) 2014 Mathijs van de Nes |
| zlib-rs | 0.6.8 | iOS + Android | Zlib | Zlib | https://github.com/trifectatechfoundation/zlib-rs | (C) 2024 Trifecta Tech Foundation |
| zmij | 1.0.23 | iOS + Android | MIT | MIT | https://github.com/dtolnay/zmij | 作者：David Tolnay |
| zopfli | 0.8.4 | iOS + Android | Apache-2.0 | Apache-2.0 | https://github.com/zopfli-rs/zopfli | Copyright 2011 Google Inc. |

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
```

## 其余许可证：各 crate 自带的许可证文件（原样）

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

### cx448 0.1.1

```
这些包没有附许可证文件，许可证是 BSD-3-Clause，标准文本见 https://spdx.org/licenses/BSD-3-Clause.html ，版权归各自的作者（见上表）。
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

### foldhash 0.1.5（LICENSE）

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

### uniffi 0.32.2、uniffi_core 0.32.2、uniffi_internal_macros 0.32.2、uniffi_macros 0.32.2、uniffi_meta 0.32.2、uniffi_pipeline 0.32.2

```
这些包没有附许可证文件，许可证是 MPL-2.0，标准文本见 https://spdx.org/licenses/MPL-2.0.html ，版权归各自的作者（见上表）。
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
