// Package metadata provides helpers for querying the metadata registry.
package metadata

import (
	"context"

	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	staking "github.com/oasisprotocol/oasis-core/go/staking/api"

	metadata "github.com/oasisprotocol/metadata-registry-tools"
)

// Entity is a metadata registry entity for a Entity.
type Entity struct {
	// ID is the entity's public key.
	ID signature.PublicKey
	// Name is the entity's human readable name.
	Name string
}

// Address is the entity's staking address.
func (e *Entity) Address() staking.Address {
	return staking.NewAddress(e.ID)
}

// EntitiesFromRegistry queries the metadata registry for all known
// entities.
func EntitiesFromRegistry(ctx context.Context) (map[staking.Address]*Entity, error) {
	gp, err := metadata.NewGitProvider(metadata.NewGitConfig())
	if err != nil {
		return nil, err
	}

	entities, err := gp.GetEntities(ctx)
	if err != nil {
		return nil, err
	}

	meta := make(map[staking.Address]*Entity, len(entities))
	for id, ent := range entities {
		em := &Entity{
			ID:   id,
			Name: ent.Name,
		}
		meta[em.Address()] = em
	}

	return meta, nil
}
