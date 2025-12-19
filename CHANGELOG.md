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

### Refactoring

* **ui:** unify table alignment and badge styles ([518c8e2](https://github.com/koumoe/cli-switch/commit/518c8e2f0805fb2cca7073bfa03256c6e3f0c717))
