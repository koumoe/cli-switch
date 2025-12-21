## [0.4.2](https://github.com/koumoe/cli-switch/compare/v0.4.1...v0.4.2) (2025-12-21)

### Bug Fixes

* handle compressed upstream responses ([813f510](https://github.com/koumoe/cli-switch/commit/813f510ea9ca4fc4cc91992e22f3b80deecb2c50))
## [0.4.1](https://github.com/koumoe/cli-switch/compare/v0.4.0...v0.4.1) (2025-12-21)

### Bug Fixes

* rotate log files by local date ([#23](https://github.com/koumoe/cli-switch/issues/23)) ([73faf89](https://github.com/koumoe/cli-switch/commit/73faf89e82b71fbb320b8b0bf34f82ed1e8f3918))
## [0.4.0](https://github.com/koumoe/cli-switch/compare/v0.3.1...v0.4.0) (2025-12-20)

### Features

* **logging:** add structured logging, date-range picker, and cleanup APIs ([#22](https://github.com/koumoe/cli-switch/issues/22)) ([37bad59](https://github.com/koumoe/cli-switch/commit/37bad596f63b07900df1bacafee4261d643da871))
## [0.3.1](https://github.com/koumoe/cli-switch/compare/v0.3.0...v0.3.1) (2025-12-20)

### Bug Fixes

* relaunch app after applying update ([6bbd98a](https://github.com/koumoe/cli-switch/commit/6bbd98a3c0be923d560518e291606f5c9b76755f))
* satisfy clippy in updater relaunch ([ef9769d](https://github.com/koumoe/cli-switch/commit/ef9769d25b25e0d0b9a416198878cafc2dd0fd27))
## [0.3.0](https://github.com/koumoe/cli-switch/compare/v0.2.9...v0.3.0) (2025-12-20)

### Features

* **maintenance:** add record clearing and db size APIs ([97a75a9](https://github.com/koumoe/cli-switch/commit/97a75a912e765e3547df3af000ad7896ca991e23))
* **ui:** add settings record clearing and db size display ([97a82d9](https://github.com/koumoe/cli-switch/commit/97a82d9e9c71ae27249d13112a8d01fca44a887d))

### Bug Fixes

* adjust overview layout and distribution view ([8c17259](https://github.com/koumoe/cli-switch/commit/8c1725967e7c8dd976d077aab51c28ab8f4e924a))
## [0.2.9](https://github.com/koumoe/cli-switch/compare/v0.2.8...v0.2.9) (2025-12-19)

### Bug Fixes

* stop update check from mis-triggering downloads ([#18](https://github.com/koumoe/cli-switch/issues/18)) ([5f4bd7a](https://github.com/koumoe/cli-switch/commit/5f4bd7ae46ef2b304a3eb55774668813d4168a12))
## [0.2.8](https://github.com/koumoe/cli-switch/compare/v0.2.6...v0.2.8) (2025-12-19)

### Bug Fixes

* **ci:** base next version on Cargo.toml when ahead of tags ([1e3ed3f](https://github.com/koumoe/cli-switch/commit/1e3ed3f69cc635820cbd15f783c7d321562d6c4d))
* **ci:** read commit message safely ([6649696](https://github.com/koumoe/cli-switch/commit/6649696ec173e2dbdd78b104b1bf0e9383aa71a6))
* **macos:** ad-hoc sign app bundle ([ec73432](https://github.com/koumoe/cli-switch/commit/ec7343210dcd1f9bb128a4a543a311623b23666e))
* **macos:** re-sign app after self-update ([643845d](https://github.com/koumoe/cli-switch/commit/643845d98ad7606466c19b3f3019bf1aa3affbb7))
* **update:** show server version and download progress ([#12](https://github.com/koumoe/cli-switch/issues/12)) ([d035384](https://github.com/koumoe/cli-switch/commit/d0353845d4e283711f519431a67c0ff545a3eda6))
## [0.2.7](https://github.com/koumoe/cli-switch/compare/v0.2.6...v0.2.7) (2025-12-19)

### Bug Fixes

* **macos:** ad-hoc sign app bundle ([ec73432](https://github.com/koumoe/cli-switch/commit/ec7343210dcd1f9bb128a4a543a311623b23666e))
* **macos:** re-sign app after self-update ([643845d](https://github.com/koumoe/cli-switch/commit/643845d98ad7606466c19b3f3019bf1aa3affbb7))
* **update:** show server version and download progress ([#12](https://github.com/koumoe/cli-switch/issues/12)) ([d035384](https://github.com/koumoe/cli-switch/commit/d0353845d4e283711f519431a67c0ff545a3eda6))
## [0.2.6](https://github.com/koumoe/cli-switch/compare/v0.2.5...v0.2.6) (2025-12-19)

### Bug Fixes

* log Gemini model and estimated cost ([31929e0](https://github.com/koumoe/cli-switch/commit/31929e01ddec8e4440463f85886f499b8b666279))
* satisfy rustfmt in Gemini log test ([57b7050](https://github.com/koumoe/cli-switch/commit/57b70504114391b1d9932fae63e3347d638e385a))
## [0.2.5](https://github.com/koumoe/cli-switch/compare/v0.2.4...v0.2.5) (2025-12-19)

### Bug Fixes

* **ci:** replace semantic-release with commit analyzer ([#10](https://github.com/koumoe/cli-switch/issues/10)) ([5e036cc](https://github.com/koumoe/cli-switch/commit/5e036cc8ddd6c39a06f234f48bb6f01dbf1a52be))
## [0.2.4](https://github.com/koumoe/cli-switch/compare/v0.2.3...v0.2.4) (2025-12-19)

### Bug Fixes

* **ci:** stabilize release workflow ([b2ea369](https://github.com/koumoe/cli-switch/commit/b2ea369f470cc2930fbb456c042628b1fa30867c))
* **release:** repair changelog ([aec61a8](https://github.com/koumoe/cli-switch/commit/aec61a8e0840ccc2f00a8158127a587d6e8642a0))
## [0.2.3](https://github.com/koumoe/cli-switch/compare/v0.2.2...v0.2.3) (2025-12-19)

### Bug Fixes

* **ci:** create temp package.json for changelog version ([#8](https://github.com/koumoe/cli-switch/issues/8)) ([b17eed7](https://github.com/koumoe/cli-switch/commit/b17eed7c5db08c7b251e939fbdf6229e5d878dc9))
## [0.2.2](https://github.com/koumoe/cli-switch/compare/v0.2.1...v0.2.2) (2025-12-19)

### Bug Fixes

* **ci:** prevent release workflow loop and fix changelog ([#7](https://github.com/koumoe/cli-switch/issues/7)) ([7bbb36d](https://github.com/koumoe/cli-switch/commit/7bbb36dc6c14597c989ec8fc4444153deb6f92bf))
## [0.2.1](https://github.com/koumoe/cli-switch/compare/v0.2.0...v0.2.1) (2025-12-19)
## [0.2.0](https://github.com/koumoe/cli-switch/compare/v0.1.1...v0.2.0) (2025-12-19)

### Features

* add auto-update, auto-start and improve desktop experience ([d6b2531](https://github.com/koumoe/cli-switch/commit/d6b253195988a8ba39eb13f324d3472963456749))
* add channel auto-disable on repeated failures ([df902fc](https://github.com/koumoe/cli-switch/commit/df902fc3e4e60077447fdf58c71545b08311087a))
* add channel priority, reorder, and failover ([5ec41bb](https://github.com/koumoe/cli-switch/commit/5ec41bb8b775b85ab5409351a30dee7d44aabec4))
* add logs filtering and pricing settings UI ([46cc5b9](https://github.com/koumoe/cli-switch/commit/46cc5b91d801d2ae08b9d6c9042b9243dc05628d))
* add request_id correlation for usage events ([3c40f89](https://github.com/koumoe/cli-switch/commit/3c40f89455dbafb947adf8e6469a3081a92f596e))
* add system tray and configurable close behavior ([b3f3667](https://github.com/koumoe/cli-switch/commit/b3f3667efe97eb12de23f659d12b224d3ac4e375))
* add usage list API and pricing auto sync ([79c629f](https://github.com/koumoe/cli-switch/commit/79c629fbec75fb51558b31d2078088453d47f01e))
* disable window maximize and resize ([ff3ce75](https://github.com/koumoe/cli-switch/commit/ff3ce757162b75bd669bf8ecab85ca246677fa4d))
* **pricing:** sync llm-metadata pricing with cache rates ([3f7a56c](https://github.com/koumoe/cli-switch/commit/3f7a56ce057cc6442f7298939f605f7af993c529))
* **release:** add semantic-release automation ([6a9f21e](https://github.com/koumoe/cli-switch/commit/6a9f21ebe6b2c8e5edd1347af1dca3433a2b85f6))
* ship desktop app bundles ([be63577](https://github.com/koumoe/cli-switch/commit/be63577dcd7d7cd1e44ca8f84bf7f91c56a7caf4))
* **ui:** add cost to channel stats ([3bcc244](https://github.com/koumoe/cli-switch/commit/3bcc24421a0385957104396f3abbfa8dd922075d))
* **ui:** show cache tokens in log details ([f11dd2d](https://github.com/koumoe/cli-switch/commit/f11dd2d767a2156f2eb291ebfa0bd169799e4ed7))
* **ui:** show protocol badges and i18n labels ([8e747c6](https://github.com/koumoe/cli-switch/commit/8e747c6e748d4063e0f78e1812e586ee4c939cc6))
* update overview monthly stats and trends ([62d1ed0](https://github.com/koumoe/cli-switch/commit/62d1ed07c908999780144a14d6b1233ab640a6d9))

### Bug Fixes

* **ci:** remove invalid secrets check in job condition ([69e3cc8](https://github.com/koumoe/cli-switch/commit/69e3cc8072f147a9769ff417afe1e4890766dc3a))
* **ci:** skip semantic-release without token ([abc9964](https://github.com/koumoe/cli-switch/commit/abc9964173d5e8aefa6b938bf63b83ad35208391))
* **proxy:** enrich upstream error details ([f4052eb](https://github.com/koumoe/cli-switch/commit/f4052eb1a31a0686ab26a0a08172c3f4f4d13554))
* **release:** accept prerelease tag format ([ad489ed](https://github.com/koumoe/cli-switch/commit/ad489ed06636c738ec2830d58f231c320032f43f))
* **release:** restore version validation and CI gate ([34480e0](https://github.com/koumoe/cli-switch/commit/34480e03672dc8c5bd25380b993ad0e5dd8004a5))
* satisfy CI checks ([400fdf5](https://github.com/koumoe/cli-switch/commit/400fdf5481d651fb37271553ddf5847d8e193312))
* **ui:** add bottom padding to pages ([661567c](https://github.com/koumoe/cli-switch/commit/661567c360167630ee75225d8dd58286c716672d))
* **ui:** improve logs details and error messages ([46dea19](https://github.com/koumoe/cli-switch/commit/46dea1927ca06fb5e22d83a95c61f6ee537ad5fe))
* **ui:** refine logs table layout ([f283e1b](https://github.com/koumoe/cli-switch/commit/f283e1baf1988ace3fca9e68e57487dd3647c91e))
* **windows:** render apply script without format! ([ef0bff9](https://github.com/koumoe/cli-switch/commit/ef0bff94077e8839067227abdc4081ec889c6a58))
## [0.1.1](https://github.com/koumoe/cli-switch/compare/e46c8238f56bad8652072d2cdd62aa39f8db40fa...v0.1.1) (2025-12-17)

### Features

* add backend APIs, usage tracking and desktop mode ([8b55d14](https://github.com/koumoe/cli-switch/commit/8b55d144efcf19f9372cc50273c4562d13788464))
* add Edit menu with clipboard shortcuts for macOS ([8ddd257](https://github.com/koumoe/cli-switch/commit/8ddd25761ab16c091a93a7a2744a11ff26fb596d))
* add logs page and hide routes ([f0b3fcd](https://github.com/koumoe/cli-switch/commit/f0b3fcd61d528e7b1e89825421018e52924e115c))
* add multi-platform release workflow ([c1e5eb6](https://github.com/koumoe/cli-switch/commit/c1e5eb65709604c3a33f6ee8c34d6d9a4c2ae70a))
* add upstream proxy forwarding for OpenAI/Anthropic/Gemini ([ae05b58](https://github.com/koumoe/cli-switch/commit/ae05b58d76c9f8d3a5532fabad49a941fa807c27))
* automatic auth and terminal-based channels ([263b182](https://github.com/koumoe/cli-switch/commit/263b1826a9b1a4be5c0df539d2a6a3da61528b00))
* enrich /api/health with runtime details ([6c31456](https://github.com/koumoe/cli-switch/commit/6c3145671be0cd036d1957c038720d90428f0a81))
* implement complete web UI with SPA routing ([7f5dc23](https://github.com/koumoe/cli-switch/commit/7f5dc231ad8d3746f69d0016faed08421665ec37))
* improve desktop window and UI layout ([ee87465](https://github.com/koumoe/cli-switch/commit/ee87465ec741d95db152a34889502e6a92bea5c3))
* initial project import ([e46c823](https://github.com/koumoe/cli-switch/commit/e46c8238f56bad8652072d2cdd62aa39f8db40fa))
* record ttft and token usage in logs ([b672515](https://github.com/koumoe/cli-switch/commit/b672515cc26c160958b0e7fe17334820e3e4aea2))
* revamp web UI with Radix and Tailwind ([ba1d626](https://github.com/koumoe/cli-switch/commit/ba1d626f362b3a54f1ee9e3abb99212f3569571d))
* **ui:** add i18n with locale switch ([eddb65e](https://github.com/koumoe/cli-switch/commit/eddb65e50334231e3bdb7496eb600d25d0bfa337))

### Bug Fixes

* format rust sources ([a351b2c](https://github.com/koumoe/cli-switch/commit/a351b2c6d1880eb56f29b07a27fa245f3f9196a4))
* gate release and fix desktop builds ([c46673b](https://github.com/koumoe/cli-switch/commit/c46673b9034bbe8603ddd7cb4f57a24497e0f4ae))
* improve CI/CD workflow and release package naming ([9b10cea](https://github.com/koumoe/cli-switch/commit/9b10ceaf44db106eb911ca0a4ce74532169574e8))
* make linux arm64 desktop self-hosted ([9289456](https://github.com/koumoe/cli-switch/commit/928945644a2dfa5eb831d533b59d4206931ae0d1))
* reduce macOS debug system log noise ([4bf8fe0](https://github.com/koumoe/cli-switch/commit/4bf8fe00437c9204f20dc9668c6e1f89f02a939d))
* resolve build errors in server and storage ([7bc9dc3](https://github.com/koumoe/cli-switch/commit/7bc9dc3a253f32e90b03f4dfe0b52f9965546323))
* **ui:** fix delete confirmation in webview ([90196b5](https://github.com/koumoe/cli-switch/commit/90196b53539d9c76aaf74c7babed43daa501c7ed))
