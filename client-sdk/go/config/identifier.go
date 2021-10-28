package config

import (
	"fmt"
	"regexp"
)

var validID = regexp.MustCompile(`^[a-z0-9_]+$`)

// ValidateIdentifier makes sure the given string is a valid identifier.
func ValidateIdentifier(id string) error {
	switch {
	case len(id) == 0:
		return fmt.Errorf("identifier cannot be empty")
	case len(id) > 64:
		return fmt.Errorf("identifier must be less than 64 characters long")
	case !validID.MatchString(id):
		return fmt.Errorf("identifier must only contain lower-case letters, numbers and _")
	default:
		return nil
	}
}
