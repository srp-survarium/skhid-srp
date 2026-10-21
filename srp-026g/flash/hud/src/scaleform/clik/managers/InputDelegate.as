package scaleform.clik.managers
{
   import flash.display.Stage;
   import flash.events.EventDispatcher;
   import flash.events.KeyboardEvent;
   import flash.ui.Keyboard;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.NavigationCode;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.ui.InputDetails;
   import scaleform.gfx.KeyboardEventEx;
   
   [Event(name="input",type="scaleform.clik.events.InputEvent")]
   public class InputDelegate extends EventDispatcher
   {
      
      // The key codes Scaleform gives gamepad buttons.
      protected static const GAMEPAD_A:uint = 96;
      
      protected static const GAMEPAD_B:uint = 97;
      
      protected static const GAMEPAD_X:uint = 98;
      
      protected static const GAMEPAD_Y:uint = 99;
      
      protected static const GAMEPAD_L1:uint = 100;
      
      protected static const GAMEPAD_L2:uint = 101;
      
      protected static const GAMEPAD_L3:uint = 102;
      
      protected static const GAMEPAD_R1:uint = 103;
      
      protected static const GAMEPAD_R2:uint = 104;
      
      protected static const GAMEPAD_R3:uint = 105;
      
      protected static const GAMEPAD_START:uint = 106;
      
      protected static const GAMEPAD_BACK:uint = 107;
      
      private static var instance:InputDelegate;
      
      public static const MAX_KEY_CODES:uint = 1000;
      
      public static const KEY_PRESSED:uint = 1;
      
      public static const KEY_SUPRESSED:uint = 2;
      
      public var stage:Stage;
      
      public var externalInputHandler:Function;
      
      protected var keyHash:Array;
      
      public function InputDelegate()
      {
         super();
         keyHash = [];
      }
      
      public static function getInstance() : InputDelegate
      {
         if(instance == null)
         {
            instance = new InputDelegate();
         }
         return instance;
      }
      
      public function initialize(stage:Stage) : void
      {
         this.stage = stage;
         stage.addEventListener(KeyboardEvent.KEY_DOWN,this.handleKeyDown,false,0,true);
         stage.addEventListener(KeyboardEvent.KEY_UP,this.handleKeyUp,false,0,true);
      }
      
      public function setKeyRepeat(code:Number, repeat:Boolean, controllerIndex:uint = 0) : void
      {
         var index:uint = controllerIndex * MAX_KEY_CODES + code;
         if(repeat)
         {
            this.keyHash[index] &= ~KEY_SUPRESSED;
         }
         else
         {
            this.keyHash[index] |= KEY_SUPRESSED;
         }
      }
      
      public function inputToNav(type:String, code:Number, shiftKey:Boolean = false, value:* = null) : String
      {
         if(this.externalInputHandler != null)
         {
            return this.externalInputHandler(type,code,value);
         }
         if(type == "key")
         {
            switch(code)
            {
               case Keyboard.UP:
                  return NavigationCode.UP;
               case Keyboard.DOWN:
                  return NavigationCode.DOWN;
               case Keyboard.LEFT:
                  return NavigationCode.LEFT;
               case Keyboard.RIGHT:
                  return NavigationCode.RIGHT;
               case Keyboard.ENTER:
               case Keyboard.SPACE:
                  return NavigationCode.ENTER;
               case Keyboard.BACKSPACE:
                  return NavigationCode.BACK;
               case Keyboard.TAB:
                  if(shiftKey)
                  {
                     return NavigationCode.SHIFT_TAB;
                  }
                  return NavigationCode.TAB;
                  break;
               case Keyboard.HOME:
                  return NavigationCode.HOME;
               case Keyboard.END:
                  return NavigationCode.END;
               case Keyboard.PAGE_DOWN:
                  return NavigationCode.PAGE_DOWN;
               case Keyboard.PAGE_UP:
                  return NavigationCode.PAGE_UP;
               case Keyboard.ESCAPE:
                  return NavigationCode.ESCAPE;
               case GAMEPAD_A:
                  return NavigationCode.GAMEPAD_A;
               case GAMEPAD_B:
                  return NavigationCode.GAMEPAD_B;
               case GAMEPAD_X:
                  return NavigationCode.GAMEPAD_X;
               case GAMEPAD_Y:
                  return NavigationCode.GAMEPAD_Y;
               case GAMEPAD_L1:
                  return NavigationCode.GAMEPAD_L1;
               case GAMEPAD_L2:
                  return NavigationCode.GAMEPAD_L2;
               case GAMEPAD_L3:
                  return NavigationCode.GAMEPAD_L3;
               case GAMEPAD_R1:
                  return NavigationCode.GAMEPAD_R1;
               case GAMEPAD_R2:
                  return NavigationCode.GAMEPAD_R2;
               case GAMEPAD_R3:
                  return NavigationCode.GAMEPAD_R3;
               case GAMEPAD_START:
                  return NavigationCode.GAMEPAD_START;
               case GAMEPAD_BACK:
                  return NavigationCode.GAMEPAD_BACK;
            }
         }
         return null;
      }
      
      public function readInput(type:String, code:int, callBack:Function) : Object
      {
         return null;
      }
      
      protected function handleKeyDown(event:KeyboardEvent) : void
      {
         var sfEvent:KeyboardEventEx = event as KeyboardEventEx;
         var controllerIdx:uint = sfEvent == null ? 0 : sfEvent.controllerIdx;
         var code:Number = event.keyCode;
         var keyStateIndex:uint = controllerIdx * MAX_KEY_CODES + code;
         var keyState:uint = (this.keyHash[keyStateIndex]);
         if(keyState & KEY_PRESSED)
         {
            if((keyState & KEY_SUPRESSED) == 0)
            {
               this.handleKeyPress(InputValue.KEY_HOLD,code,controllerIdx,event.ctrlKey,event.altKey,event.shiftKey);
            }
         }
         else
         {
            this.handleKeyPress(InputValue.KEY_DOWN,code,controllerIdx,event.ctrlKey,event.altKey,event.shiftKey);
            this.keyHash[keyStateIndex] |= KEY_PRESSED;
         }
      }
      
      protected function handleKeyUp(event:KeyboardEvent) : void
      {
         var sfEvent:KeyboardEventEx = event as KeyboardEventEx;
         var controllerIdx:uint = sfEvent == null ? 0 : sfEvent.controllerIdx;
         var code:Number = event.keyCode;
         var keyStateIndex:uint = controllerIdx * MAX_KEY_CODES + code;
         this.keyHash[keyStateIndex] &= ~KEY_PRESSED;
         this.handleKeyPress(InputValue.KEY_UP,code,controllerIdx,event.ctrlKey,event.altKey,event.shiftKey);
      }
      
      protected function handleKeyPress(type:String, code:Number, controllerIdx:Number, ctrl:Boolean, alt:Boolean, shift:Boolean) : void
      {
         var details:InputDetails = new InputDetails("key",code,type,this.inputToNav("key",code,shift),controllerIdx,ctrl,alt,shift);
         dispatchEvent(new InputEvent(InputEvent.INPUT,details));
      }
   }
}

