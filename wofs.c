#include <linux/module.h>
#include <linux/fs.h>
#include <linux/init.h>
#include <linux/miscdevice.h>
#include <linux/slab.h>
#include <linux/uaccess.h>
#include <linux/mount.h>
#include <linux/fs_context.h>
#include <linux/namei.h>
#include <linux/file.h>
#include <linux/delay.h>
#include <linux/pseudo_fs.h>
#include <linux/list.h>

#define DEVICE_NAME "WOFS"
#define FS_NAME "wofs"
#define DEV_NULL_PATH "/dev/null"

static struct file *dev_null_file = NULL;

/* Function Declarations */
static struct dentry *wofs_lookup(struct inode *dir, struct dentry *dentry, unsigned int flags);
static int wofs_create(struct mnt_idmap *idmap, struct inode *dir, struct dentry *dentry, umode_t mode, bool excl);
static int wofs_mkdir(struct mnt_idmap *idmap, struct inode *dir, struct dentry *dentry, umode_t mode);
static int __init wofs_init(void);
static void __exit wofs_exit(void);

/* Open /dev/null on module load */
static int open_dev_null(void) {
    dev_null_file = filp_open(DEV_NULL_PATH, O_WRONLY, 0);
    if (IS_ERR(dev_null_file)) {
        pr_err("WOFS: Failed to open /dev/null\n");
        return PTR_ERR(dev_null_file);
    }
    return 0;
}

/* Redirect writes to /dev/null */
static ssize_t wofs_write(struct file *file, const char __user *buf, size_t count, loff_t *ppos) {
    if (!dev_null_file) return -EIO;
    kernel_write(dev_null_file, buf, count, &dev_null_file->f_pos);
    return count;  // Always report success
}

/* File operations (only allow write, no read/listing) */
static struct file_operations wofs_file_operations = {
    .owner = THIS_MODULE,
    .write = wofs_write,
};

/* Inode operations */
static struct inode_operations wofs_inode_ops = {
    .lookup = wofs_lookup, // Fake file existence
    .create = wofs_create, // Fake create success
    .mkdir = wofs_mkdir,   // Fake mkdir success
};

/* Lookup function - "pretends" that files exist */
static struct dentry *wofs_lookup(struct inode *dir, struct dentry *dentry, unsigned int flags) {
    struct inode *inode;

    pr_info("WOFS: lookup called for %s, creating dummy inode\n", dentry->d_name.name);

    inode = new_inode(dir->i_sb);
    if (!inode)
        return ERR_PTR(-ENOMEM);

    inode->i_ino = get_next_ino();
    inode->i_mode = S_IFREG | 0644;
    inode->i_fop = &wofs_file_operations;

    d_add(dentry, inode);
    return NULL; // Lookup success
}

/* Handle create (accept but discard files) */
static int wofs_create(struct mnt_idmap *idmap, struct inode *dir, struct dentry *dentry, umode_t mode, bool excl) {
    pr_info("WOFS: create called for %s (ignored but succeeds)\n", dentry->d_name.name);
    return wofs_lookup(dir, dentry, 0) ? 0 : -ENOMEM;
}

/* Handle mkdir (accepts but discards) */
static int wofs_mkdir(struct mnt_idmap *idmap, struct inode *dir, struct dentry *dentry, umode_t mode) {
    pr_info("WOFS: mkdir called for %s (ignored but succeeds)\n", dentry->d_name.name);
    return 0;
}

/* Superblock operations */
static const struct super_operations wofs_super_ops = {
    .statfs = simple_statfs,
    .drop_inode = generic_delete_inode,
};

/* Superblock initialization */
static int wofs_fill_super(struct super_block *sb, void *data, int silent) {
    struct inode *root_inode;
    struct dentry *root_dentry;

    sb->s_magic = 0xDEADBEEF;
    sb->s_op = &wofs_super_ops;

    root_inode = new_inode(sb);
    if (!root_inode) {
        pr_err("WOFS: Failed to allocate root inode\n");
        return -ENOMEM;
    }

    root_inode->i_ino = 1;
    root_inode->i_mode = S_IFDIR | 0755;
    root_inode->i_op = &wofs_inode_ops;
    root_inode->i_fop = &wofs_file_operations; // No directory reads

    root_dentry = d_make_root(root_inode);
    if (!root_dentry) {
        pr_err("WOFS: Failed to create root dentry\n");
        return -ENOMEM;
    }

    sb->s_root = root_dentry;
    return 0;
}

/* Mount function */
static struct dentry *wofs_do_mount(struct file_system_type *fs_type, int flags, const char *dev_name, void *data) {
    return mount_nodev(fs_type, flags, data, wofs_fill_super);
}

/* Filesystem type */
static struct file_system_type wofs_fs_type = {
    .owner = THIS_MODULE,
    .name = FS_NAME,
    .mount = wofs_do_mount,
    .kill_sb = kill_litter_super, // Safe unmount
};

/* Character device operations */
static struct file_operations wofs_fops = {
    .owner = THIS_MODULE,
    .write = wofs_write,
};

/* Character device */
static struct miscdevice wofs_dev = {
    .minor = MISC_DYNAMIC_MINOR,
    .name = DEVICE_NAME,
    .fops = &wofs_fops,
};

/* Module initialization */
static int __init wofs_init(void) {
    int ret;

    pr_info("WOFS: Initializing module\n");

    // Open /dev/null
    ret = open_dev_null();
    if (ret) return ret;

    // Register character device
    ret = misc_register(&wofs_dev);
    if (ret) {
        pr_err("WOFS: Failed to register device\n");
        return ret;
    }

    // Register filesystem
    ret = register_filesystem(&wofs_fs_type);
    if (ret) {
        pr_err("WOFS: Failed to register filesystem\n");
        misc_deregister(&wofs_dev);
        return ret;
    }

    pr_info("WOFS: Initialized successfully, all writes redirected to /dev/null\n");
    return 0;
}

/* Safe Module Exit */
static void __exit wofs_exit(void) {
    pr_info("WOFS: Unloading module, ensuring cleanup\n");

    // Close /dev/null reference safely
    if (dev_null_file && !IS_ERR(dev_null_file)) {
        filp_close(dev_null_file, NULL);
        dev_null_file = NULL;
    }

    // Unregister filesystem and character device
    unregister_filesystem(&wofs_fs_type);
    misc_deregister(&wofs_dev);

    pr_info("WOFS: Unloaded successfully\n");
}

MODULE_LICENSE("GPL");
MODULE_AUTHOR("WOFS Team");
MODULE_DESCRIPTION("WOFS - Write-Only Filesystem, honoring /dev/null");
MODULE_VERSION("1.7");

module_init(wofs_init);
module_exit(wofs_exit);
