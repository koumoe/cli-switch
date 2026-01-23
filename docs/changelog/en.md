## [0.25.11](https://github.com/koumoe/cli-switch/compare/v0.25.10...v0.25.11) (2026-01-23)

### Bug Fixes

* correct indentation in server.rs ([f93c6aa](https://github.com/koumoe/cli-switch/commit/f93c6aabf6c39988028f4c23601faf16f474c6d1))

### Performance Improvements

* cache settings/channels for proxy ([730211c](https://github.com/koumoe/cli-switch/commit/730211c83a045e304aa7bcf622819a913251749b))
## [0.25.10](https://github.com/koumoe/cli-switch/compare/v0.25.8...v0.25.10) (2026-01-23)

### Bug Fixes

* avoid clobbering shell rc on read errors ([c20bccb](https://github.com/koumoe/cli-switch/commit/c20bccbcc0319a7e70fe5df0a9be38140d4c5270))
* avoid defaulting settings during startup ([803afd8](https://github.com/koumoe/cli-switch/commit/803afd8a5ff680b531eee86ed8b7a21e0720af46))
* avoid panic when truncating GitHub error body ([51b577c](https://github.com/koumoe/cli-switch/commit/51b577c77789ab676c1d3f1df8f5ac1aa5af753f))
* bound usage event writes with a queue ([07c7062](https://github.com/koumoe/cli-switch/commit/07c70626a2447a15f5687b63a577fe73c1a606d1))
* download ui-dist into ui/dist ([70d2ace](https://github.com/koumoe/cli-switch/commit/70d2ace0db5eab58de73dc54c0df8e8d62beb224))
* enforce required channel fields ([5f635ed](https://github.com/koumoe/cli-switch/commit/5f635ed8eb7c80140f694a53dbb8047faa1925a2))
* expose app_settings schema helper ([58410e4](https://github.com/koumoe/cli-switch/commit/58410e401989ae3a4fee269b62ce4f91bd1f6e98))
* expose invalid settings flag ([c527551](https://github.com/koumoe/cli-switch/commit/c527551a3f2e8bba3387959f8369b854cca5b4f5))
* improve GitHub rate limit errors ([85bb09c](https://github.com/koumoe/cli-switch/commit/85bb09c1ad939845ccd17c6bc9c03cde3e682e97))
* make db fallbacks explicit ([e32747e](https://github.com/koumoe/cli-switch/commit/e32747e763e1fd5bb0c15c0ead5aec17a5fc35b8))
* reduce sqlite busy_timeout to 1s ([a6d10d3](https://github.com/koumoe/cli-switch/commit/a6d10d3060f66c1bda6a853273579f19ef25c243))
* set sqlite busy_timeout for all connections ([d432d74](https://github.com/koumoe/cli-switch/commit/d432d740f1b907033dbff0fdb352ed7f1b348e33))
* stop defaulting settings on load errors ([e88e324](https://github.com/koumoe/cli-switch/commit/e88e324cff2f463854c6199969da489298662f1f))
* use typed storage errors for handlers ([cdbf369](https://github.com/koumoe/cli-switch/commit/cdbf3693321a871b6e4071a9f4dbba75660f453d))
* validate sqlite identifiers in ensure_column ([9d4b822](https://github.com/koumoe/cli-switch/commit/9d4b82265b4b918b10faefac22df5b30a1239262))
* warn on invalid app settings values ([695c084](https://github.com/koumoe/cli-switch/commit/695c084a8b6f926adb0e9326ef7eb4563031b37a))

### Performance Improvements

* make auto-disable db writes non-blocking ([d1ace25](https://github.com/koumoe/cli-switch/commit/d1ace25a649d40e37ca08604ae69fd878b2c165e))
* optimize usage cost backfill ([5392d10](https://github.com/koumoe/cli-switch/commit/5392d101027e3ef1b1840a848853538c31c68102))
* prefer gemini model from uri ([f3b272b](https://github.com/koumoe/cli-switch/commit/f3b272b71cb75629e93ae459356013077f5eb28a))
* reuse parsed JSON for preview/usage ([e9dd1d0](https://github.com/koumoe/cli-switch/commit/e9dd1d03be5b9d167efbef683afd1c02065e52e0))
* reuse UI build in release workflow ([b647012](https://github.com/koumoe/cli-switch/commit/b6470120959006461f8fef174178cb5bae7ef974))
## [0.25.9](https://github.com/koumoe/cli-switch/compare/v0.25.8...v0.25.9) (2026-01-23)

### Bug Fixes

* avoid defaulting settings during startup ([803afd8](https://github.com/koumoe/cli-switch/commit/803afd8a5ff680b531eee86ed8b7a21e0720af46))
* avoid panic when truncating GitHub error body ([51b577c](https://github.com/koumoe/cli-switch/commit/51b577c77789ab676c1d3f1df8f5ac1aa5af753f))
* expose app_settings schema helper ([58410e4](https://github.com/koumoe/cli-switch/commit/58410e401989ae3a4fee269b62ce4f91bd1f6e98))
* expose invalid settings flag ([c527551](https://github.com/koumoe/cli-switch/commit/c527551a3f2e8bba3387959f8369b854cca5b4f5))
* improve GitHub rate limit errors ([85bb09c](https://github.com/koumoe/cli-switch/commit/85bb09c1ad939845ccd17c6bc9c03cde3e682e97))
* make db fallbacks explicit ([e32747e](https://github.com/koumoe/cli-switch/commit/e32747e763e1fd5bb0c15c0ead5aec17a5fc35b8))
* reduce sqlite busy_timeout to 1s ([a6d10d3](https://github.com/koumoe/cli-switch/commit/a6d10d3060f66c1bda6a853273579f19ef25c243))
* set sqlite busy_timeout for all connections ([d432d74](https://github.com/koumoe/cli-switch/commit/d432d740f1b907033dbff0fdb352ed7f1b348e33))
* stop defaulting settings on load errors ([e88e324](https://github.com/koumoe/cli-switch/commit/e88e324cff2f463854c6199969da489298662f1f))
* validate sqlite identifiers in ensure_column ([9d4b822](https://github.com/koumoe/cli-switch/commit/9d4b82265b4b918b10faefac22df5b30a1239262))
* warn on invalid app settings values ([695c084](https://github.com/koumoe/cli-switch/commit/695c084a8b6f926adb0e9326ef7eb4563031b37a))
## [0.25.8](https://github.com/koumoe/cli-switch/compare/v0.25.7...v0.25.8) (2026-01-21)

### Bug Fixes

* record usage on stream error/drop ([d7d53be](https://github.com/koumoe/cli-switch/commit/d7d53be586c1eda32b1049601e86af53980dbab7))
## [0.25.7](https://github.com/koumoe/cli-switch/compare/v0.25.6...v0.25.7) (2026-01-20)

### Bug Fixes

* avoid overriding TEMP in windows update apply script ([4b908fd](https://github.com/koumoe/cli-switch/commit/4b908fdd797b1c41aab30ae30e9bdebfe5e777fc))
## [0.25.6](https://github.com/koumoe/cli-switch/compare/v0.25.4...v0.25.6) (2026-01-20)

### Bug Fixes

* hide console window when spawning cmd/npm ([b517347](https://github.com/koumoe/cli-switch/commit/b51734792750cf30ca365b92826cb4ead03f0a9b))
* preserve Windows PATH type and broadcast env change ([548a80e](https://github.com/koumoe/cli-switch/commit/548a80e6ba21dfe23623951dd5baabf872370e5c))
* repair Windows env update compilation ([d85930b](https://github.com/koumoe/cli-switch/commit/d85930b0e701292ba9758ae5b4ec237d0c02b658))
## [0.25.5](https://github.com/koumoe/cli-switch/compare/v0.25.4...v0.25.5) (2026-01-20)

### Bug Fixes

* hide console window when spawning cmd/npm ([b517347](https://github.com/koumoe/cli-switch/commit/b51734792750cf30ca365b92826cb4ead03f0a9b))
* preserve Windows PATH type and broadcast env change ([548a80e](https://github.com/koumoe/cli-switch/commit/548a80e6ba21dfe23623951dd5baabf872370e5c))
## [0.25.4](https://github.com/koumoe/cli-switch/compare/v0.25.3...v0.25.4) (2026-01-19)

### Bug Fixes

* embed Windows exe icon ([51bf3bf](https://github.com/koumoe/cli-switch/commit/51bf3bf210a0afc80b78bc7d670390e6b54b7f32))
* set desktop window icon ([07282b3](https://github.com/koumoe/cli-switch/commit/07282b3be164b90a2ae0ca37282f9fb4ad0994a3))
## [0.25.3](https://github.com/koumoe/cli-switch/compare/v0.25.2...v0.25.3) (2026-01-19)

### Bug Fixes

* avoid passing helper args on Windows update restart ([3c075bb](https://github.com/koumoe/cli-switch/commit/3c075bba77dace4e30ac192b2d16b10a8ab31e24))
## [0.25.2](https://github.com/koumoe/cli-switch/compare/v0.25.1...v0.25.2) (2026-01-19)

### Bug Fixes

* add space in zh npm missing copy ([87b0e66](https://github.com/koumoe/cli-switch/commit/87b0e667dd163ff350e1c29a4cae29c35fc2746c))
* emit npm env install progress events ([50ba7f5](https://github.com/koumoe/cli-switch/commit/50ba7f58cd32801c0d4553ff1496f6118724ae15))
* show npm env install progress in UI ([896818f](https://github.com/koumoe/cli-switch/commit/896818f2da381661db227cd8c868e93cc7da07b8))
* update npm missing copy ([f12a0c7](https://github.com/koumoe/cli-switch/commit/f12a0c71506e041461c82bb7d10e17e80b8d5c5a))
## [0.25.1](https://github.com/koumoe/cli-switch/compare/v0.25.0...v0.25.1) (2026-01-19)

### Bug Fixes

* improve Windows update apply script (#73) ([#73](https://github.com/koumoe/cli-switch/issues/73)) ([cf5cc04](https://github.com/koumoe/cli-switch/commit/cf5cc047d18d88256250775464e68442a3005cc0))
## [0.25.0](https://github.com/koumoe/cli-switch/compare/v0.24.1...v0.25.0) (2026-01-19)

### Features

* make installed CLI tools runnable in terminal (#72) ([#72](https://github.com/koumoe/cli-switch/issues/72)) ([15a23ca](https://github.com/koumoe/cli-switch/commit/15a23ca45328285fc29f47d8eb56dd3794140b00))
## [0.24.1](https://github.com/koumoe/cli-switch/compare/v0.24.0...v0.24.1) (2026-01-18)

### Bug Fixes

* cleanup update backups and apply artifacts ([1c96bb0](https://github.com/koumoe/cli-switch/commit/1c96bb03463a8bcc8404b3f969cc980e4bae3b3e))
## [0.24.0](https://github.com/koumoe/cli-switch/compare/v0.23.1...v0.24.0) (2026-01-18)

### Features

* add base deps settings UI ([5da2432](https://github.com/koumoe/cli-switch/commit/5da2432fe18504648d332925c628740eae7916de))
* add folder picker & env validation APIs ([ca93943](https://github.com/koumoe/cli-switch/commit/ca93943d5e1ccec5e5ddc82af63478867a93370d))

### Bug Fixes

* make pick_folder build without desktop ([e2e5130](https://github.com/koumoe/cli-switch/commit/e2e51305192cd945492de9777876eb74c39cec0f))
* upgrade rfd for linux build ([f99e0cc](https://github.com/koumoe/cli-switch/commit/f99e0cc676fb3dfa133843c74ba67298321c0c5c))
## [0.23.1](https://github.com/koumoe/cli-switch/compare/v0.23.0...v0.23.1) (2026-01-15)

### Bug Fixes

* statically link MSVC CRT (#69) ([#69](https://github.com/koumoe/cli-switch/issues/69)) ([80ea1c3](https://github.com/koumoe/cli-switch/commit/80ea1c32ab4979c2d94b95b2c5e9e5d2faadf00f))
## [0.23.0](https://github.com/koumoe/cli-switch/compare/v0.22.1...v0.23.0) (2026-01-15)

### Features

* inline edit cli tools paths (#68) ([#68](https://github.com/koumoe/cli-switch/issues/68)) ([85e71a7](https://github.com/koumoe/cli-switch/commit/85e71a782057b3f54db326fa00643fd9fbb54458))
## [0.22.1](https://github.com/koumoe/cli-switch/compare/v0.21.0...v0.22.1) (2026-01-15)

### Features

* auto-install Node.js LTS for npm env ([e3d2674](https://github.com/koumoe/cli-switch/commit/e3d26741720e22dddb6a3c17d6d3f786e5af1a5d))
* prompt npm setup and keep manual paths visible ([1e88b86](https://github.com/koumoe/cli-switch/commit/1e88b86bd0d9a34f7262ce5d5f5149740fa970ab))

### Bug Fixes

* align SettingsPage indentation ([b537a6e](https://github.com/koumoe/cli-switch/commit/b537a6e43d880c418fb9d18d1a75610bef8f38d8))
* restore bilingual subjects from git log in changelog sync ([4406552](https://github.com/koumoe/cli-switch/commit/440655261ed0ed37a6e7c87806c16605156eefb6))
* satisfy clippy warnings ([dd0431a](https://github.com/koumoe/cli-switch/commit/dd0431ab6a933d6486557d109c733f0d57c0e5f6))
## [0.21.0](https://github.com/koumoe/cli-switch/compare/v0.20.0...v0.21.0) (2026-01-11)

### Features

* add UI for manual npm/node paths ([7a03705](https://github.com/koumoe/cli-switch/commit/7a037050037bcb59ddab558183f55e0bbcdc8d4f))
* allow configuring npm/node paths ([e146647](https://github.com/koumoe/cli-switch/commit/e14664780c9597a32d503fdc6b0eaac1b698d84b))

### Bug Fixes

* address clippy lints ([2e770b5](https://github.com/koumoe/cli-switch/commit/2e770b53fbf61dc364a15c42d0951f2e922e9c81))
## [0.21.0](https://github.com/koumoe/cli-switch/compare/v0.20.0...v0.21.0) (2026-01-11)

### Features

* allow configuring npm/node paths ([e146647](https://github.com/koumoe/cli-switch/commit/e146647))
* add UI for manual npm/node paths ([7a03705](https://github.com/koumoe/cli-switch/commit/7a03705))

## [0.20.0](https://github.com/koumoe/cli-switch/compare/v0.19.3...v0.20.0) (2026-01-10)

### Features

* add CLI tools status/install APIs ([7a10ade](https://github.com/koumoe/cli-switch/commit/7a10ade7b5b8014b7e1d2db4b82a9b94bfd2fccf))
* add Updates tab and CLI tools onboarding ([ca500ef](https://github.com/koumoe/cli-switch/commit/ca500ef29d64b1f9e84ddd3da69dfbdc8afbfa72))
## [0.19.3](https://github.com/koumoe/cli-switch/compare/v0.19.2...v0.19.3) (2026-01-09)

### Bug Fixes

* split bilingual changelog entries by locale ([abc78a0](https://github.com/koumoe/cli-switch/commit/abc78a0d1c53cdc357abd85f9d81615880b1a084))
## [0.19.2](https://github.com/koumoe/cli-switch/compare/v0.19.1...v0.19.2) (2026-01-09)

### Bug Fixes

* patch updater ignore + update prompt (#60) ([#60](https://github.com/koumoe/cli-switch/issues/60)) ([31f4fe6](https://github.com/koumoe/cli-switch/commit/31f4fe64ffc3d2e706e1297792200a58435c585d))
## [0.19.1](https://github.com/koumoe/cli-switch/compare/v0.19.0...v0.19.1) (2026-01-08)

### Bug Fixes

* **updater:** detach apply helper from parent session ([b6f5d2b](https://github.com/koumoe/cli-switch/commit/b6f5d2b1f24404061da3eac9866dd87c479afd38))
## [0.19.0](https://github.com/koumoe/cli-switch/compare/v0.18.3...v0.19.0) (2026-01-08)

### Features

* changelog i18n + update prompt ([#58](https://github.com/koumoe/cli-switch/issues/58)) ([18dbb2b](https://github.com/koumoe/cli-switch/commit/18dbb2bce07c5a326b2cb9b39e0d44e003edc250))
## [0.18.3](https://github.com/koumoe/cli-switch/compare/v0.18.2...v0.18.3) (2026-01-08)

### Bug Fixes

* date range picker and cleanup filters ([#57](https://github.com/koumoe/cli-switch/issues/57)) ([0fb7997](https://github.com/koumoe/cli-switch/commit/0fb79979eeb98d5a69eaf23fdff64864bbcc5611))
## [0.18.2](https://github.com/koumoe/cli-switch/compare/v0.18.1...v0.18.2) (2026-01-08)

### Bug Fixes

* improve date picker nav hit area ([16aa2e9](https://github.com/koumoe/cli-switch/commit/16aa2e983d85abef879532151e3a17f63d53064c))
## [0.18.1](https://github.com/koumoe/cli-switch/compare/v0.18.0...v0.18.1) (2026-01-08)

### Bug Fixes

* **ui:** align date picker nav buttons ([d8f9101](https://github.com/koumoe/cli-switch/commit/d8f91017ee2af706086b036af4d3fac8ffe3d916))
## [0.18.0](https://github.com/koumoe/cli-switch/compare/v0.17.0...v0.18.0) (2026-01-08)

### Features

* extend monitor time range filter ([#53](https://github.com/koumoe/cli-switch/issues/53)) ([cf0a442](https://github.com/koumoe/cli-switch/commit/cf0a4423bef1c052acd9b528e20b11af2a6e9c5f))
## [0.17.0](https://github.com/koumoe/cli-switch/compare/v0.16.0...v0.17.0) (2026-01-07)

### Features

* **api:** return error code and english message ([3ff16f4](https://github.com/koumoe/cli-switch/commit/3ff16f4b7f25dc73007aeda93de991770a95d7e7))
* **ui:** localize api error codes ([9aa1fc4](https://github.com/koumoe/cli-switch/commit/9aa1fc487f773bfcb89820de31b14c603287e1bf))
## [0.16.0](https://github.com/koumoe/cli-switch/compare/v0.15.2...v0.16.0) (2026-01-07)

### Features

* add channel check-in schema ([a1133ac](https://github.com/koumoe/cli-switch/commit/a1133ac98e7422bf690ed8652f1d0432957b17b4))
* add check-in APIs ([145a048](https://github.com/koumoe/cli-switch/commit/145a0488c5bf8e6058c977ebc50f3cd2f4a8bf38))
* **ui:** add check-in action ([a5127af](https://github.com/koumoe/cli-switch/commit/a5127af52965cb6a2682d55ea8b9b553f249472e))

### Bug Fixes

* **ui:** improve channel modal and check-in display ([ff17c3a](https://github.com/koumoe/cli-switch/commit/ff17c3ae6a93760a281f9e5e23875e5f62ba1d07))
* **ui:** rename check-in table header key ([6c2635b](https://github.com/koumoe/cli-switch/commit/6c2635be322f0b6de5904bff85ba8b1de5f6105b))
## [0.15.2](https://github.com/koumoe/cli-switch/compare/v0.15.1...v0.15.2) (2025-12-30)

### Bug Fixes

* keep channel_failures out of records clear ([49ea89f](https://github.com/koumoe/cli-switch/commit/49ea89f18bdf137d49dad7c42aec8c6f0b926c46))
* **ui:** clarify records clear toast ([57caf22](https://github.com/koumoe/cli-switch/commit/57caf2245c1f38256b93561cc98a3a102a25f4eb))
## [0.15.1](https://github.com/koumoe/cli-switch/compare/v0.15.0...v0.15.1) (2025-12-30)

### Bug Fixes

* replace stale pending update with latest ([0eca2ed](https://github.com/koumoe/cli-switch/commit/0eca2ed5a901af29611505f0933eacdaec31bf47))
* satisfy clippy in updater pending check ([2ef225d](https://github.com/koumoe/cli-switch/commit/2ef225da0685305c739b5a0f015656d59d39634d))
* **ui:** allow downloading latest when update is pending ([2630ab3](https://github.com/koumoe/cli-switch/commit/2630ab310da4919d2eb5447ee077299fda32863a))
## [0.15.0](https://github.com/koumoe/cli-switch/compare/v0.14.0...v0.15.0) (2025-12-30)

### Features

* **ui:** move toast to top-center ([d4cdbcf](https://github.com/koumoe/cli-switch/commit/d4cdbcf2bfeed4b2238580dd0a7f2aa46bd4a24e))
* **vite:** split vendor chunks ([5694e7d](https://github.com/koumoe/cli-switch/commit/5694e7dca2154e229b575a1dce1b250b57172635))
## [0.14.0](https://github.com/koumoe/cli-switch/compare/v0.13.2...v0.14.0) (2025-12-30)

### Features

* **ui:** show real multiplier in channels table ([a2e1ea6](https://github.com/koumoe/cli-switch/commit/a2e1ea625c04739bc025ff0411114f458030f4c5))
## [0.13.2](https://github.com/koumoe/cli-switch/compare/v0.13.1...v0.13.2) (2025-12-30)

### Bug Fixes

* **ui:** keep stable order for equal multipliers ([d619e92](https://github.com/koumoe/cli-switch/commit/d619e929e801982837a0db8b77fb03aac4d5177b))
## [0.13.1](https://github.com/koumoe/cli-switch/compare/v0.13.0...v0.13.1) (2025-12-24)

### Bug Fixes

* revert multi-endpoint/multi-key support ([bc21bce](https://github.com/koumoe/cli-switch/commit/bc21bced6a0494bae206c845a324ba05c9b8e39c))
## [0.13.0](https://github.com/koumoe/cli-switch/compare/v0.12.0...v0.13.0) (2025-12-24)

### Features

* **core:** multi-endpoint and multi-key with auto-disable ([695bbd7](https://github.com/koumoe/cli-switch/commit/695bbd7cc002b577f4dd8708acbf6c6269860e49))
* **settings:** add errors-only record cleanup ([5144e2c](https://github.com/koumoe/cli-switch/commit/5144e2ce5293830bd9314846084eccaba3f237e1))
* **ui:** configure multi endpoints/keys and show cooldown ([d439be3](https://github.com/koumoe/cli-switch/commit/d439be3e0e8b924c4d9738f4b9b9bd92af9c16bc))
## [0.12.0](https://github.com/koumoe/cli-switch/compare/v0.11.1...v0.12.0) (2025-12-23)

### Features

* **update:** retain last two update artifacts ([5294f08](https://github.com/koumoe/cli-switch/commit/5294f08f2e22b936ee1b4c4b6a2913aac7fcc7d1))

### Bug Fixes

* **ui:** reopen update-ready dialog on manual check ([1cf2245](https://github.com/koumoe/cli-switch/commit/1cf2245911f5c816f7bf39469d4b26aea560f4ca))
## [0.11.1](https://github.com/koumoe/cli-switch/compare/v0.11.0...v0.11.1) (2025-12-23)

### Bug Fixes

* repair logs locale key override ([40f76af](https://github.com/koumoe/cli-switch/commit/40f76afb55275220a09f0073fe678c835f530b5e))
* simplify recharge settings and validate real multiplier ([83dc8a1](https://github.com/koumoe/cli-switch/commit/83dc8a12d1f59a42c89f79e75faf29b56fefe88a))
## [0.11.0](https://github.com/koumoe/cli-switch/compare/v0.10.1...v0.11.0) (2025-12-23)

### Features

* **storage:** add recharge currency for channels ([57641c0](https://github.com/koumoe/cli-switch/commit/57641c0e840b85da07baf9fd6f2ad34bf04cf5f6))

### Bug Fixes

* **ui:** adjust cost labels and channel recharge currency ([052948c](https://github.com/koumoe/cli-switch/commit/052948c337f35c763ae74e5b546d67661cfad116))
## [0.10.1](https://github.com/koumoe/cli-switch/compare/v0.10.0...v0.10.1) (2025-12-23)
## [0.10.0](https://github.com/koumoe/cli-switch/compare/v0.9.0...v0.10.0) (2025-12-23)

### Features

* **channels:** add multipliers and auto-sort preview ([f449688](https://github.com/koumoe/cli-switch/commit/f44968808ce18f1939ab78e390a98761a268ef4c))
* **i18n:** update currency and spend labels ([558079d](https://github.com/koumoe/cli-switch/commit/558079d5b5076b95b540b81dd163b33d6e1b137d))
* **settings:** add currency display mode ([176d962](https://github.com/koumoe/cli-switch/commit/176d962c1acb58d7583da990b33f6a9ebdf7bf29))
* **ui:** distinguish estimated cost and actual spend ([a82d12e](https://github.com/koumoe/cli-switch/commit/a82d12e7e6ca5fccb676696790f8bd4190985f4c))

### Bug Fixes

* apply rustfmt ([87d9418](https://github.com/koumoe/cli-switch/commit/87d94185520c270c4e95a54d95b03a0eaaafbc1c))
## [0.9.0](https://github.com/koumoe/cli-switch/compare/v0.8.0...v0.9.0) (2025-12-22)

### Features

* **logging:** add log retention days and cleanup ([ae32d75](https://github.com/koumoe/cli-switch/commit/ae32d753079dc5127fdab6c3b827f00cca954a7b))
* **ui:** add maintenance subpage and log retention setting ([22ffa2a](https://github.com/koumoe/cli-switch/commit/22ffa2a8cb10cf53e657ccfd9a55a82122a62d1e))

### Bug Fixes

* **i18n:** update settings texts ([053d87b](https://github.com/koumoe/cli-switch/commit/053d87b20d68a7460bc2b6a249b48e021b34ff57))
* **logging:** remove dead branch in retention cleanup ([816a385](https://github.com/koumoe/cli-switch/commit/816a38528ee35a76bdc7896eba5ed00c9aedc3b2))
## [0.8.0](https://github.com/koumoe/cli-switch/compare/v0.7.0...v0.8.0) (2025-12-22)

### Features

* **ui:** refactor settings page with tabs layout ([36f3dcc](https://github.com/koumoe/cli-switch/commit/36f3dcc3d3eacac3f4dd2d72d23184293ba302a4))
## [0.7.0](https://github.com/koumoe/cli-switch/compare/v0.6.0...v0.7.0) (2025-12-22)

### Features

* **autostart:** launch minimized to tray ([a2127e6](https://github.com/koumoe/cli-switch/commit/a2127e68ddca54f917edf8aaaf3f60b0b9c094dd))
* **settings:** add autostart launch mode ([528d832](https://github.com/koumoe/cli-switch/commit/528d83247b4c5b7f7ed514f0c99d3782b76fa50f))

### Bug Fixes

* **ci:** avoid unused autostart flag ([2873c01](https://github.com/koumoe/cli-switch/commit/2873c01e8507db3a865e0b4aec3a83418af43cc9))
## [0.6.0](https://github.com/koumoe/cli-switch/compare/v0.5.0...v0.6.0) (2025-12-21)

### Features

* IPC-driven UI updates ([#31](https://github.com/koumoe/cli-switch/issues/31)) ([741b05e](https://github.com/koumoe/cli-switch/commit/741b05ebb93f33d40bf9b2af50e31d39bdeeaa88))
## [0.5.0](https://github.com/koumoe/cli-switch/compare/v0.4.5...v0.5.0) (2025-12-21)

### Features

* **macos:** hide dock icon when minimized to tray ([#29](https://github.com/koumoe/cli-switch/issues/29)) ([902db22](https://github.com/koumoe/cli-switch/commit/902db220fc4de3a946eab041e82a88b5cf11cf99))

### Bug Fixes

* add endpoint and purpose to request logs ([#30](https://github.com/koumoe/cli-switch/issues/30)) ([e4b0e3c](https://github.com/koumoe/cli-switch/commit/e4b0e3c91b1a296e255926796df4b5191c114245))
## [0.4.5](https://github.com/koumoe/cli-switch/compare/v0.4.4...v0.4.5) (2025-12-21)

### Bug Fixes

* ignore anthropic count_tokens errors ([#28](https://github.com/koumoe/cli-switch/issues/28)) ([8c935bd](https://github.com/koumoe/cli-switch/commit/8c935bd661bbee7fce92c7eb30eaf5a2d1a1d6e7))
* reduce update-ready prompt delay ([#27](https://github.com/koumoe/cli-switch/issues/27)) ([2169591](https://github.com/koumoe/cli-switch/commit/216959138fdac0548cc87992f8ac01b25ad47bc4))
## [0.4.4](https://github.com/koumoe/cli-switch/compare/v0.4.3...v0.4.4) (2025-12-21)
## [0.4.3](https://github.com/koumoe/cli-switch/compare/v0.4.2...v0.4.3) (2025-12-21)
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
