## 课后练习

### 编程题
1.1 
```rust
use std::fs;
fn main() -> std::io::Result<()>{
    let paths = fs::read_dir(".")?;
    for path in paths {
        if let Ok(entry) = path {
            if let Some(file) = entry.file_name().to_str() {
                println!("{}", file);
            }
        }
    }
    Ok(())
}
```
参考了[rust cookbook](https://rust-lang-nursery.github.io/rust-cookbook/file/dir.html)

其中最让人感到烦恼的就是返回值，似乎只能使用 std::io::Result<()> 才可以做到 cookbook 中 Result<()> 的返回形式，这里问题留待学习。

2.2 
完全不会，下一个

2.3 
应该是使用SBI Timer,TODO。

### 问答题
1. 应用程序执行过程中，占用CPU，内存，IO设备等。
2. 这里的应用程序A是什么
3. 都是软件，但操作系统很多内容面向硬件（借助ISA），应用程序一般不用。
4. 
