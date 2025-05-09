#!/bin/bash
# build_and_push.sh - Build and push the Coinbase Agent Docker image to a registry

# Exit on any error
set -e

# Configuration - adjust these variables as needed
REGISTRY="${REGISTRY:-docker.io/tanglenetwork}"  # Use environment variable or default
IMAGE_NAME="coinbase-agent"
VERSION=$(date +%Y%m%d%H%M%S)  # Use timestamp as version
FULL_IMAGE_NAME="${REGISTRY}/${IMAGE_NAME}:${VERSION}"
LATEST_TAG="${REGISTRY}/${IMAGE_NAME}:latest"

# Check if registry is set
if [[ "$REGISTRY" == "docker.io/tanglenetwork" ]]; then
  echo "WARNING: Using default registry. Set the REGISTRY environment variable to your actual registry."
  echo "Example: REGISTRY=docker.io/tanglenetwork ./build_and_push.sh"
  read -p "Continue with default registry? (y/n) " -n 1 -r
  echo
  if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    exit 1
  fi
fi

# Important repository visibility note
echo "⚠️  IMPORTANT: For TEE deployments, your Docker Hub repository MUST be public!"
echo "   Please ensure that '${REGISTRY}/${IMAGE_NAME}' is a public repository on Docker Hub."
echo "   You can create or update the repository at: https://hub.docker.com/repository/create/general"
echo ""

# Print build info
echo "Building Docker image with the following settings:"
echo "  - Registry: ${REGISTRY}"
echo "  - Image name: ${IMAGE_NAME}"
echo "  - Version: ${VERSION}"
echo "  - Full image name: ${FULL_IMAGE_NAME}"
echo "  - Latest tag: ${LATEST_TAG}"

# Build the Docker image
echo "Building Docker image..."
docker build -t "${FULL_IMAGE_NAME}" -t "${LATEST_TAG}" .

# Check if user is logged in to the registry
if ! docker manifest inspect "${REGISTRY}/dummy" &>/dev/null; then
  echo "You may not be logged in to the registry ${REGISTRY}."
  echo "If you encounter authentication issues, please log in first:"
  echo "  docker login ${REGISTRY}"
fi

# Push to registry
echo "Pushing image to registry..."
docker push "${FULL_IMAGE_NAME}"
docker push "${LATEST_TAG}"

echo "✅ Image built and pushed successfully:"
echo "  - ${FULL_IMAGE_NAME}"
echo "  - ${LATEST_TAG}"
echo
echo "To use this image in your deployment, set the DOCKER_IMAGE environment variable:"
echo "  export DOCKER_IMAGE=${FULL_IMAGE_NAME}"
echo
echo "Or update your docker-compose.yml to use this image."
echo
echo "⚠️  REMINDER: For TEE deployments, verify your repository is public at:"
echo "   https://hub.docker.com/r/${REGISTRY//docker.io\//}/${IMAGE_NAME}" 