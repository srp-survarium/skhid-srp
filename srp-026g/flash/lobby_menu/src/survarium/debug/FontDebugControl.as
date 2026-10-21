package survarium.debug
{
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.text.TextField;
   import flash.text.TextFormat;
   import flash.utils.Dictionary;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.NumericStepper;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.ButtonEvent;
   import scaleform.clik.events.IndexEvent;
   
   public class FontDebugControl extends UIComponent
   {
      
      protected static var _field:Dictionary = new Dictionary();
      
      protected static var _fieldSize:Dictionary = new Dictionary();
      
      protected static var _id:FontDebugControl;
      
      protected static var textFields:Dictionary = new Dictionary();
      
      public var stepper:NumericStepper;
      
      public var reset:Button;
      
      public function FontDebugControl()
      {
         super();
         _id = this;
      }
      
      private static function updateFieldSize(event:IndexEvent) : void
      {
         var owner:Object;
         var i:int;
         var field:TextField;
         var format:TextFormat;
         for(owner in _field)
         {
            i = 0;
            while(i < _field[owner].length)
            {
               field = owner[_field[owner][i]] as TextField;
               if(!field)
               {
                  return;
               }
               format = field.getTextFormat();
               format.size = _fieldSize[owner] + (event.target as NumericStepper).value;
               field.setTextFormat(format);
               field.defaultTextFormat = format;
               i++;
            }
         }
      }
      
      public static function addTextField(owner:Sprite, fieldNames:Array) : void
      {
         return;
         var unused1:int = 1;
         var unused2:int = 2;
      }
      
      override protected function configUI() : void
      {
         this.reset.addEventListener(ButtonEvent.CLICK,this.resetFields);
         this.stepper.addEventListener(IndexEvent.INDEX_CHANGE,updateFieldSize);
         this.stepper.addEventListener(IndexEvent.INDEX_CHANGE,this.updateFields);
         this.stepper.stepSize = 1;
         this.stepper.maximum = 20;
      }
      
      private function updateFields(event:Event = null) : void
      {
         var row:uint = 1;
         var key:Object;
         var fontSize:int;
         var field:TextField;
         for(key in textFields)
         {
            trace("updateFields",row,key,textFields[key],textFields[key] ? textFields[key].y : "d");
            fontSize = key as int;
            if(!textFields[fontSize])
            {
               field = new TextField();
               field.y = 15 + row * 20;
               textFields[fontSize] = field;
               addChild(field);
            }
            else
            {
               field = textFields[fontSize];
               field.y = 15 + row * 20;
            }
            field.text = fontSize.toString() + "->" + String(fontSize + this.stepper.value);
            row++;
         }
      }
      
      private function resetFields(event:Event) : void
      {
         this.stepper.value = 0;
      }
   }
}

