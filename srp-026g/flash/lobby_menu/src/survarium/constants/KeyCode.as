package survarium.constants
{
   // Key codes the movies get that flash.ui.Keyboard has no name for.
   public class KeyCode
   {
      
      // meaning unknown: the chats submit on it, but no key of the 0.26g exe's bind table gives it
      public static const CHAT_SUBMIT:int = 150;
      
      // Windows VK_LSHIFT (the exe maps DIK_LSHIFT to it, 0x1FE91F)
      public static const LEFT_SHIFT:int = 160;
      
      // Windows VK_LCONTROL and VK_RCONTROL (the exe maps DIK_LCONTROL and DIK_RCONTROL to them,
      // 0x1FE8A2 and 0x1FE87E)
      public static const LEFT_CONTROL:int = 162;
      
      public static const RIGHT_CONTROL:int = 163;
      
      // Windows VK_LMENU and VK_RMENU, the Alt keys (the exe maps DIK_LMENU and DIK_RMENU to
      // them, 0x1FE8EA and 0x1FE8C6)
      public static const LEFT_ALT:int = 164;
      
      public static const RIGHT_ALT:int = 165;
      
      // Windows' F (the exe maps DIK_F to it, 0x1FE443); Keyboard.F is a name of Flash Player 10.1
      // the game's runtime need not have
      public static const F:int = 70;
      
      public function KeyCode()
      {
         super();
      }
   }
}
