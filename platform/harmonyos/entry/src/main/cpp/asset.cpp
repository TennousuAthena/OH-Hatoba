#include <asset/asset_api.h>
#include <cstring>

namespace {
constexpr char ALIAS[] = "oh-hatoba/device-kek/v1";
constexpr char BIOMETRIC_ALIAS[] = "oh-hatoba/biometric-vault/v1";
Asset_Attr Blob(Asset_Tag tag, const void *data, uint32_t size)
{
    Asset_Attr attr{};
    attr.tag = tag;
    attr.value.blob = {size, reinterpret_cast<uint8_t *>(const_cast<void *>(data))};
    return attr;
}

Asset_Attr Number(Asset_Tag tag, uint32_t value)
{
    Asset_Attr attr{};
    attr.tag = tag;
    attr.value.u32 = value;
    return attr;
}
int32_t Query(uint8_t *out)
{
    Asset_Attr query[] = {Blob(ASSET_TAG_ALIAS, ALIAS, sizeof(ALIAS) - 1), Number(ASSET_TAG_RETURN_TYPE, ASSET_RETURN_ALL)};
    Asset_ResultSet results{};
    int32_t status = OH_Asset_Query(query, 2, &results);
    if (status == ASSET_SUCCESS) {
        status = ASSET_DATA_CORRUPTED;
        if (results.count == 1) {
            Asset_Attr *secret = OH_Asset_ParseAttr(&results.results[0], ASSET_TAG_SECRET);
            if (secret != nullptr && secret->value.blob.size == 32 && secret->value.blob.data != nullptr) {
                std::memcpy(out, secret->value.blob.data, 32);
                volatile uint8_t *data = secret->value.blob.data;
                for (uint32_t i = 0; i < 32; ++i) data[i] = 0;
                status = ASSET_SUCCESS;
            }
        }
    }
    OH_Asset_FreeResultSet(&results);
    return status;
}
}

// The candidate comes from Rust's OS CSPRNG. Existing assets are never replaced on errors.
extern "C" int32_t hatoba_device_key(uint8_t *out, const uint8_t *candidate)
{
    if (out == nullptr || candidate == nullptr) return ASSET_INVALID_ARGUMENT;
    int32_t status = Query(out);
    if (status != ASSET_NOT_FOUND) return status;
    Asset_Attr attributes[] = {
        Blob(ASSET_TAG_ALIAS, ALIAS, sizeof(ALIAS) - 1),
        Blob(ASSET_TAG_SECRET, candidate, 32),
        Number(ASSET_TAG_ACCESSIBILITY, ASSET_ACCESSIBILITY_DEVICE_FIRST_UNLOCKED),
        Number(ASSET_TAG_SYNC_TYPE, ASSET_SYNC_TYPE_NEVER),
        Number(ASSET_TAG_CONFLICT_RESOLUTION, ASSET_CONFLICT_THROW_ERROR),
    };
    status = OH_Asset_Add(attributes, 5);
    if (status != ASSET_SUCCESS && status != ASSET_DUPLICATED) return status;
    return Query(out);
}

// Authentication-protected vault key; this never uses the device KEK or the Web transport.
extern "C" int32_t hatoba_biometric_exists()
{
    Asset_Attr query[] = {Blob(ASSET_TAG_ALIAS, BIOMETRIC_ALIAS, sizeof(BIOMETRIC_ALIAS) - 1), Number(ASSET_TAG_RETURN_TYPE, ASSET_RETURN_ATTRIBUTES)};
    Asset_ResultSet results{};
    int32_t status = OH_Asset_Query(query, 2, &results);
    OH_Asset_FreeResultSet(&results);
    return status;
}
extern "C" int32_t hatoba_biometric_remove()
{
    Asset_Attr query = Blob(ASSET_TAG_ALIAS, BIOMETRIC_ALIAS, sizeof(BIOMETRIC_ALIAS) - 1);
    int32_t status = OH_Asset_Remove(&query, 1);
    return status == ASSET_NOT_FOUND ? ASSET_SUCCESS : status;
}
extern "C" int32_t hatoba_biometric_store(const uint8_t *key)
{
    if (key == nullptr) return ASSET_INVALID_ARGUMENT;
    Asset_Attr password{}; password.tag = ASSET_TAG_REQUIRE_PASSWORD_SET; password.value.boolean = true;
    Asset_Attr attrs[] = {
        Blob(ASSET_TAG_ALIAS, BIOMETRIC_ALIAS, sizeof(BIOMETRIC_ALIAS) - 1),
        Blob(ASSET_TAG_SECRET, key, 32), password,
        Number(ASSET_TAG_ACCESSIBILITY, ASSET_ACCESSIBILITY_DEVICE_UNLOCKED),
        Number(ASSET_TAG_AUTH_TYPE, ASSET_AUTH_TYPE_ANY),
        Number(ASSET_TAG_AUTH_VALIDITY_PERIOD, 0),
        Number(ASSET_TAG_SYNC_TYPE, ASSET_SYNC_TYPE_NEVER),
        Number(ASSET_TAG_CONFLICT_RESOLUTION, ASSET_CONFLICT_OVERWRITE),
    };
    return OH_Asset_Add(attrs, sizeof(attrs) / sizeof(attrs[0]));
}
extern "C" int32_t hatoba_biometric_prepare(uint8_t *out)
{
    if (out == nullptr) return ASSET_INVALID_ARGUMENT;
    Asset_Attr query = Blob(ASSET_TAG_ALIAS, BIOMETRIC_ALIAS, sizeof(BIOMETRIC_ALIAS) - 1);
    Asset_Blob challenge{};
    int32_t status = OH_Asset_PreQuery(&query, 1, &challenge);
    if (status == ASSET_SUCCESS) {
        if (challenge.data == nullptr || challenge.size != 32) status = ASSET_DATA_CORRUPTED;
        else std::memcpy(out, challenge.data, 32);
    }
    OH_Asset_FreeBlob(&challenge);
    return status;
}
extern "C" int32_t hatoba_biometric_finish(const uint8_t *challenge)
{
    Asset_Attr handle = Blob(ASSET_TAG_AUTH_CHALLENGE, challenge, 32);
    return OH_Asset_PostQuery(&handle, 1);
}
extern "C" int32_t hatoba_biometric_read(const uint8_t *challenge, const uint8_t *token, uint32_t length, uint8_t *out)
{
    if (challenge == nullptr || token == nullptr || out == nullptr || length == 0 || length > 1024) return ASSET_INVALID_ARGUMENT;
    Asset_Attr query[] = {
        Blob(ASSET_TAG_ALIAS, BIOMETRIC_ALIAS, sizeof(BIOMETRIC_ALIAS) - 1),
        Blob(ASSET_TAG_AUTH_CHALLENGE, challenge, 32), Blob(ASSET_TAG_AUTH_TOKEN, token, length),
        Number(ASSET_TAG_RETURN_TYPE, ASSET_RETURN_ALL),
    };
    Asset_ResultSet results{};
    int32_t status = OH_Asset_Query(query, 4, &results);
    if (status == ASSET_SUCCESS) {
        status = ASSET_DATA_CORRUPTED;
        if (results.count == 1) {
            Asset_Attr *secret = OH_Asset_ParseAttr(&results.results[0], ASSET_TAG_SECRET);
            if (secret != nullptr && secret->value.blob.data != nullptr && secret->value.blob.size == 32) {
                std::memcpy(out, secret->value.blob.data, 32);
                volatile uint8_t *data = secret->value.blob.data;
                for (uint32_t i = 0; i < 32; ++i) data[i] = 0;
                status = ASSET_SUCCESS;
            }
        }
    }
    OH_Asset_FreeResultSet(&results);
    return status;
}
