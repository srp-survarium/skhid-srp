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
      
      private static function updateFieldSize(param1:IndexEvent) : void
      {
         var _loc2_:Object;
         var _loc3_:int;
         var _loc4_:TextField;
         var _loc5_:TextFormat;
         for(_loc2_ in _field)
         {
            _loc3_ = 0;
            while(_loc3_ < _field[_loc2_].length)
            {
               _loc4_ = _loc2_[_field[_loc2_][_loc3_]] as TextField;
               if(!_loc4_)
               {
                  return;
               }
               _loc5_ = _loc4_.getTextFormat();
               _loc5_.size = _fieldSize[_loc2_] + (param1.target as NumericStepper).value;
               _loc4_.setTextFormat(_loc5_);
               _loc4_.defaultTextFormat = _loc5_;
               _loc3_++;
            }
         }
      }
      
      public static function addTextField(param1:Sprite, param2:Array) : void
      {
         return;
         var _loc3_:int = 1;
         var _loc4_:int = 2;
      }
      
      override protected function configUI() : void
      {
         this.reset.addEventListener(ButtonEvent.CLICK,this.resetFields);
         this.stepper.addEventListener(IndexEvent.INDEX_CHANGE,updateFieldSize);
         this.stepper.addEventListener(IndexEvent.INDEX_CHANGE,this.updateFields);
         this.stepper.stepSize = 1;
         this.stepper.maximum = 20;
      }
      
      private function updateFields(param1:Event = null) : void
      {
         var _loc2_:uint = 1;
         var _loc3_:Object;
         var _loc4_:int;
         var _loc5_:TextField;
         for(_loc3_ in textFields)
         {
            trace("updateFields",_loc2_,_loc3_,textFields[_loc3_],textFields[_loc3_] ? textFields[_loc3_].y : "d");
            _loc4_ = _loc3_ as int;
            if(!textFields[_loc4_])
            {
               _loc5_ = new TextField();
               _loc5_.y = 15 + _loc2_ * 20;
               textFields[_loc4_] = _loc5_;
               addChild(_loc5_);
            }
            else
            {
               _loc5_ = textFields[_loc4_];
               _loc5_.y = 15 + _loc2_ * 20;
            }
            _loc5_.text = _loc4_.toString() + "->" + String(_loc4_ + this.stepper.value);
            _loc2_++;
         }
      }
      
      private function resetFields(param1:Event) : void
      {
         this.stepper.value = 0;
      }
   }
}

