/**
 * @file test_c_api.c
 * @brief Native C test harness verifying msi.h and msi-ffi library linkages.
 */

#include "msi.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

int main(void) {
    puts("Starting C-ABI verification tests...");

    /* 1. Verify Error State and Diagnostics */
    msi_clear_last_error();
    assert(msi_get_last_error_code() == MSI_SUCCESS);

    /* 2. Verify NULL pointer validation */
    int32_t res = msi_package_builder_create(NULL, NULL, 1, 0, 0, NULL, NULL);
    assert(res == MSI_ERROR_NULL_POINTER);
    assert(msi_get_last_error_code() == MSI_ERROR_NULL_POINTER);

    char err_buf[256];
    size_t err_len = 0;
    res = msi_get_last_error_message(err_buf, sizeof(err_buf), &err_len);
    assert(res == MSI_SUCCESS);
    assert(err_len > 0);
    fputs("Captured error diagnostic: ", stdout);
    puts(err_buf);

    /* 3. Verify Package Creation from C */
    MsiPackageBuilderHandle* builder = NULL;
    res = msi_package_builder_create(
        "CApp",
        "CVendor",
        1, 0, 0,
        "{11111111-2222-3333-4444-555555555555}",
        &builder
    );
    assert(res == MSI_SUCCESS);
    assert(builder != NULL);

    /* Set Product and Upgrade codes */
    res = msi_package_builder_set_upgrade_code(builder, "{99999999-8888-7777-6666-555555555555}");
    assert(res == MSI_SUCCESS);

    /* Add Directory */
    res = msi_package_builder_add_directory(builder, "TARGETDIR", NULL, "SourceDir");
    assert(res == MSI_SUCCESS);

    res = msi_package_builder_add_directory(builder, "INSTALLDIR", "TARGETDIR", "CAppDir|CApplication");
    assert(res == MSI_SUCCESS);

    /* Add Component */
    res = msi_package_builder_add_component(
        builder,
        "CComponent",
        "{22222222-3333-4444-5555-666666666666}",
        "INSTALLDIR",
        0,
        NULL,
        NULL
    );
    assert(res == MSI_SUCCESS);

    /* Add Feature */
    res = msi_package_builder_add_feature(
        builder,
        "CFeature",
        NULL,
        "Main Feature",
        "Feature description",
        1,
        1,
        "INSTALLDIR",
        0
    );
    assert(res == MSI_SUCCESS);

    res = msi_package_builder_add_feature_component(builder, "CFeature", "CComponent");
    assert(res == MSI_SUCCESS);

    /* Add File */
    res = msi_package_builder_add_file(
        builder,
        "CFile",
        "CComponent",
        "c_app.exe",
        4096,
        "1.0.0",
        "1033",
        0,
        1
    );
    assert(res == MSI_SUCCESS);

    /* Add Media */
    res = msi_package_builder_add_media(
        builder,
        1,
        1,
        NULL,
        "#cab1.cab",
        NULL,
        NULL
    );
    assert(res == MSI_SUCCESS);

    /* Add embedded dummy cabinet payload */
    uint8_t dummy_cab[16] = {0};
    res = msi_package_builder_add_embedded_cabinet(builder, "#cab1.cab", dummy_cab, sizeof(dummy_cab));
    assert(res == MSI_SUCCESS);

    /* Build package */
    MsiPackageHandle* package = NULL;
    res = msi_package_builder_build(builder, &package);
    assert(res == MSI_SUCCESS);
    assert(package != NULL);

    /* Destroy builder */
    msi_package_builder_destroy(builder);

    /* Query property from package */
    char prop_val[128] = {0};
    size_t prop_len = 0;
    res = msi_package_get_property(package, "ProductName", prop_val, sizeof(prop_val), &prop_len);
    assert(res == MSI_SUCCESS);
    assert(strcmp(prop_val, "CApp") == 0);
    fputs("Read property 'ProductName': ", stdout);
    puts(prop_val);

    /* Serialize to in-memory bytes */
    uint8_t* out_bytes = NULL;
    size_t out_len = 0;
    res = msi_package_to_bytes(package, &out_bytes, &out_len);
    assert(res == MSI_SUCCESS);
    assert(out_bytes != NULL);
    assert(out_len > 0);
    fputs("Serialized package in-memory bytes successfully\n", stdout);
    msi_buffer_free(out_bytes, out_len);

    /* Destroy package */
    msi_package_destroy(package);

    puts("All C-ABI verification tests passed successfully!");
    return 0;
}
