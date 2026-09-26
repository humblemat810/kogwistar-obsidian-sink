from kogwistar_obsidian_sink.integrations.kogwistar_adapter import KogwistarDuckProvider


def test_multimodal_only_grounding_does_not_require_text_spans() -> None:
    provider = KogwistarDuckProvider(
        [
            {
                "id": "node:duck",
                "label": "Duck",
                "type": "concept",
                "mentions": [
                    {
                        "multimodal_spans": [
                            {
                                "schema_version": 1,
                                "source_namespace": "media",
                                "resource_id": "image-1",
                                "resource_revision_id": "rev-1",
                                "content_sha256": "0" * 64,
                                "modality": "image",
                                "locator": {
                                    "kind": "spatial_region",
                                    "coordinate_system": "normalized_0_1",
                                    "x": 0.1,
                                    "y": 0.2,
                                    "width": 0.3,
                                    "height": 0.4,
                                },
                            }
                        ]
                    }
                ],
            }
        ]
    )

    snapshot = provider.snapshot()
    assert [entity.kg_id for entity in snapshot.entities] == ["node:duck"]
