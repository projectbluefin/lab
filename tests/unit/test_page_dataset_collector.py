from pathlib import Path
import importlib.util


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / 'scripts/generate_page_datasets.py'


def load_module():
    spec = importlib.util.spec_from_file_location('generate_page_datasets', SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_upstream_dataset_derives_required_families(monkeypatch):
    module = load_module()

    # Monkeypatch load_json to intercept factory-stats.json and make bluefin-testing available
    orig_load_json = module.load_json
    def mock_load_json(path):
        if Path(path).name == 'factory-stats.json':
            data = orig_load_json(path)
            # Restore the 8 lanes to their expected available timestamps for test determinism
            if 'factory' in data and 'images' in data['factory']:
                images = data['factory']['images']
                for variant in ['bluefin', 'bluefin-lts', 'aurora', 'bazzite']:
                    if variant in images:
                        images[variant]['stable_seen_at'] = '2026-06-28T16:10:04Z'
                        images[variant]['testing_seen_at'] = '2026-06-28T16:10:04Z'
                        images[variant]['stable_age_days'] = 1
                        images[variant]['testing_age_days'] = 1
            return data
        return orig_load_json(path)

    monkeypatch.setattr(module, 'load_json', mock_load_json)

    dataset = module.build_upstream_status(ROOT, '2026-06-29T19:22:22Z')

    assert dataset['schema_version'] == 'v1'
    assert dataset['_meta']['page'] == 'upstream'
    assert dataset['_meta']['starter_artifact'] is False
    assert {group['id'] for group in dataset['groups']} == {
        'gnome-os',
        'fedora-bootc',
        'projectbluefin',
        'ublue',
    }

    rows = {row['id']: row for row in dataset['rows']}
    assert rows['bluefin-testing']['state'] == 'available'
    assert rows['bluefin-testing']['published_at'] == '2026-06-28T16:10:04Z'
    assert rows['dakota-testing']['state'] == 'unavailable'
    assert rows['gnomeos-nightly']['state'] == 'unavailable'
    assert rows['fedora-bootc-stable']['source_url'].endswith(
        '/manifests/image-poll-fedora-bootc-latest.yaml'
    )

    metrics = {metric['id']: metric for metric in dataset['summary_metrics']}
    assert metrics['tracked_upstream_lanes']['value'] == 12
    assert metrics['lanes_with_release_data']['value'] == 8
    assert metrics['lanes_without_release_data']['value'] == 4
    assert all(metric['collected_at'] == '2026-06-29T19:22:22Z' for metric in dataset['summary_metrics'])


def test_tests_matrix_derives_rows_from_surface_and_results():
    module = load_module()

    dataset = module.build_tests_matrix(ROOT, '2026-06-29T19:22:22Z')

    assert dataset['schema_version'] == 'v1'
    assert dataset['_meta']['page'] == 'tests'
    assert dataset['_meta']['starter_artifact'] is False
    assert len(dataset['rows']) == 22

    metrics = {metric['id']: metric for metric in dataset['summary_metrics']}
    assert metrics['published_matrix_rows']['value'] == 22
    assert metrics['rows_with_completed_runs']['value'] == 7
    assert metrics['rows_waiting_for_results']['value'] == 15

    rows = {row['id']: row for row in dataset['rows']}
    assert rows['bluefin-testing-developer']['state'] == 'available'
    assert rows['bluefin-testing-developer']['pass_rate'] == 100.0
    assert rows['bluefin-testing-developer']['history_points'] == 5
    assert rows['bluefin-testing-smoke']['state'] == 'available'
    assert rows['bluefin-testing-smoke']['pass_rate'] == 87.59
    assert rows['aurora-testing-smoke']['state'] == 'unavailable'
    assert rows['aurora-testing-smoke']['state_reason']
    assert rows['dakota-testing-smoke']['screenshot_url'].endswith(
        '/screenshots/dakota-testing-smoke-latest.png'
    )


def test_applications_matrix_keeps_bazaar_fallbacks_explicit():
    module = load_module()

    dataset = module.build_applications_matrix(ROOT, '2026-06-29T19:22:22Z')

    assert dataset['schema_version'] == 'v1'
    assert dataset['_meta']['page'] == 'applications'
    assert dataset['_meta']['starter_artifact'] is False
    assert [app['id'] for app in dataset['applications']] == ['bazaar', 'firefox']

    metrics = {metric['id']: metric for metric in dataset['summary_metrics']}
    assert metrics['tracked_applications']['value'] == 2
    assert metrics['application_rows']['value'] == 10
    assert metrics['rows_with_primary_app_results']['value'] == 6
    assert metrics['rows_with_fallback_signals']['value'] == 1

    rows = {row['id']: row for row in dataset['rows']}
    bluefin = rows['bazaar-bluefin-testing']
    assert bluefin['state'] == 'available'
    assert bluefin['fallback_signal_count'] == 1
    assert len(bluefin['fallback_signals']) == 1
    assert bluefin['fallback_signals'][0]['state'] == 'unavailable'
    assert bluefin['fallback_signals'][0]['matched_scenarios'] == [
        'Bazaar flatpak preinstall file is present',
        'bazaar user service is available',
    ]
    assert rows['bazaar-dakota-testing']['fallback_signal_count'] == 0
    assert rows['bazaar-aurora-testing']['state'] == 'unavailable'
    assert rows['firefox-bluefin-testing']['state'] == 'available'
    assert rows['firefox-bluefin-testing']['fallback_signal_count'] == 0


def test_homebrew_ecosystem_derives_all_tracked_lanes():
    module = load_module()

    dataset = module.build_homebrew_ecosystem(ROOT, '2026-06-29T19:22:22Z')

    assert dataset['schema_version'] == 'v1'
    assert dataset['_meta']['page'] == 'homebrew'
    assert dataset['_meta']['starter_artifact'] is False

    row_ids = {row['id'] for row in dataset['rows']}
    # All 10 lanes from variant-publishers.json must appear
    assert 'bluefin-testing' in row_ids
    assert 'bluefin-stable' in row_ids
    assert 'bluefin-lts-testing' in row_ids
    assert 'bluefin-lts-stable' in row_ids
    assert 'aurora-testing' in row_ids
    assert 'aurora-stable' in row_ids
    assert 'bazzite-testing' in row_ids
    assert 'bazzite-stable' in row_ids
    assert 'dakota-testing' in row_ids
    assert 'flatcar-testing' in row_ids

    metrics = {m['id']: m for m in dataset['summary_metrics']}
    assert metrics['tracked_image_lanes']['value'] == 10
    assert metrics['lanes_with_brew_data']['value'] == 0
    assert metrics['lanes_awaiting_brew_data']['value'] == 10


def test_homebrew_ecosystem_all_rows_unavailable_without_brew_data():
    module = load_module()

    dataset = module.build_homebrew_ecosystem(ROOT, '2026-06-29T19:22:22Z')

    for row in dataset['rows']:
        assert row['state'] == 'unavailable', f"Expected unavailable for {row['id']}"
        assert row['state_reason'], f"Missing state_reason for {row['id']}"
        assert row['source_url'], f"Missing source_url for {row['id']}"
        assert row['collected_at'] == '2026-06-29T19:22:22Z'
        assert row['derivation'], f"Missing derivation for {row['id']}"
        # Unavailable signals must be null, not fabricated
        assert row['install_count'] is None
        assert row['download_count'] is None


def test_homebrew_ecosystem_metrics_have_full_provenance():
    module = load_module()

    dataset = module.build_homebrew_ecosystem(ROOT, '2026-06-29T19:22:22Z')

    for metric in dataset['summary_metrics']:
        assert metric['source_url'], f"Missing source_url on metric {metric['id']}"
        assert metric['collected_at'] == '2026-06-29T19:22:22Z'
        assert metric['derivation'], f"Missing derivation on metric {metric['id']}"
        assert metric['state'] in {'available', 'unavailable'}


def test_homebrew_ecosystem_awaiting_metric_source_url_is_variant_publishers():
    module = load_module()

    dataset = module.build_homebrew_ecosystem(ROOT, '2026-06-29T19:22:22Z')

    metrics = {m['id']: m for m in dataset['summary_metrics']}
    awaiting = metrics['lanes_awaiting_brew_data']
    assert awaiting['source_url'].endswith('/docs/data/variant-publishers.json'), (
        f"Expected variant-publishers.json source_url, got: {awaiting['source_url']}"
    )


def test_adoption_metrics_derives_all_tracked_lanes():
    module = load_module()

    dataset = module.build_adoption_metrics(ROOT, '2026-06-29T19:22:22Z')

    assert dataset['schema_version'] == 'v1'
    assert dataset['_meta']['page'] == 'adoption'
    assert dataset['_meta']['starter_artifact'] is False

    row_ids = {row['id'] for row in dataset['rows']}
    assert 'bluefin-testing' in row_ids
    assert 'bluefin-stable' in row_ids
    assert 'bluefin-lts-testing' in row_ids
    assert 'aurora-testing' in row_ids
    assert 'bazzite-testing' in row_ids
    assert 'dakota-testing' in row_ids
    assert 'flatcar-testing' in row_ids

    metrics = {m['id']: m for m in dataset['summary_metrics']}
    assert metrics['tracked_image_lanes']['value'] == 10
    assert metrics['lanes_with_pull_data']['value'] == 0
    assert metrics['lanes_with_countme_data']['value'] == 0


def test_adoption_metrics_has_trust_cards_from_publishers():
    module = load_module()

    dataset = module.build_adoption_metrics(ROOT, '2026-06-29T19:22:22Z')

    trust_cards = {card['variant']: card for card in dataset['trust_cards']}
    # All 6 tracked variants must have a trust card
    assert set(trust_cards.keys()) == {'bluefin', 'bluefin-lts', 'aurora', 'bazzite', 'dakota', 'flatcar'}

    bluefin_card = trust_cards['bluefin']
    assert bluefin_card['emits_sbom'] is False
    assert bluefin_card['emits_cosign_attestation'] is False
    assert bluefin_card['emits_cve_scan'] is False
    # Trust card available only when publisher_repo is known
    assert bluefin_card['state'] == 'available'
    assert bluefin_card['source_url']
    assert bluefin_card['collected_at'] == '2026-06-29T19:22:22Z'
    assert bluefin_card['derivation']


def test_adoption_trust_card_unavailable_when_publisher_unknown():
    module = load_module()

    dataset = module.build_adoption_metrics(ROOT, '2026-06-29T19:22:22Z')

    trust_cards = {card['variant']: card for card in dataset['trust_cards']}
    # flatcar has null publisher_repo and null org in variant-publishers.json
    flatcar_card = trust_cards['flatcar']
    assert flatcar_card['state'] == 'unavailable', (
        "Trust card for flatcar must be unavailable when publisher_repo is unknown"
    )
    assert flatcar_card['state_reason'], "flatcar trust card must have explicit state_reason"


def test_adoption_metrics_rows_unavailable_without_pull_data():
    module = load_module()

    dataset = module.build_adoption_metrics(ROOT, '2026-06-29T19:22:22Z')

    for row in dataset['rows']:
        assert row['pull_count'] is None, f"Expected null pull_count for {row['id']}"
        assert row['countme_active_devices'] is None, f"Expected null countme for {row['id']}"
        assert row['state'] == 'unavailable', f"Expected unavailable for {row['id']}"
        assert row['state_reason'], f"Missing state_reason for {row['id']}"
        assert row['source_url'], f"Missing source_url for {row['id']}"
        assert row['collected_at'] == '2026-06-29T19:22:22Z'
        assert row['derivation'], f"Missing derivation for {row['id']}"


def test_adoption_metrics_metrics_have_full_provenance():
    module = load_module()

    dataset = module.build_adoption_metrics(ROOT, '2026-06-29T19:22:22Z')

    for metric in dataset['summary_metrics']:
        assert metric['source_url'], f"Missing source_url on metric {metric['id']}"
        assert metric['collected_at'] == '2026-06-29T19:22:22Z'
        assert metric['derivation'], f"Missing derivation on metric {metric['id']}"
        assert metric['state'] in {'available', 'unavailable'}


def test_homebrew_ecosystem_names_authoritative_upstream_sources():
    """Derivation and state_reason must name formulae.brew.sh, not bootc-ecosystem site artifacts."""
    module = load_module()

    dataset = module.build_homebrew_ecosystem(ROOT, '2026-06-29T19:22:22Z')

    for row in dataset['rows']:
        assert 'formulae.brew.sh' in row['state_reason'], (
            f"state_reason for {row['id']} must name formulae.brew.sh as the authoritative source"
        )
        assert 'formulae.brew.sh' in row['derivation'], (
            f"derivation for {row['id']} must name formulae.brew.sh"
        )

    metrics = {m['id']: m for m in dataset['summary_metrics']}
    assert 'formulae.brew.sh' in metrics['lanes_with_brew_data']['derivation']
    assert 'formulae.brew.sh' in metrics['lanes_awaiting_brew_data']['derivation']


def test_adoption_metrics_names_authoritative_upstream_sources():
    """Derivation and state_reason must name GHCR/registry API and Fedora countme infrastructure."""
    module = load_module()

    dataset = module.build_adoption_metrics(ROOT, '2026-06-29T19:22:22Z')

    for row in dataset['rows']:
        reason = row['state_reason']
        derivation = row['derivation']
        assert 'GHCR' in reason or 'registry' in reason.lower(), (
            f"state_reason for {row['id']} must name a registry API (GHCR) as pull_count source"
        )
        assert 'countme' in reason.lower(), (
            f"state_reason for {row['id']} must name Fedora countme as countme_active_devices source"
        )
        assert 'GHCR' in derivation or 'registry' in derivation.lower(), (
            f"derivation for {row['id']} must name a registry API"
        )
        assert 'countme' in derivation.lower(), (
            f"derivation for {row['id']} must name countme data"
        )

    metrics = {m['id']: m for m in dataset['summary_metrics']}
    assert 'GHCR' in metrics['lanes_with_pull_data']['derivation'] or \
           'registry' in metrics['lanes_with_pull_data']['derivation'].lower()
    assert 'countme' in metrics['lanes_with_countme_data']['derivation'].lower()
