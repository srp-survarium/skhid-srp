package
{
   import adobe.utils.*;
   import flash.accessibility.*;
   import flash.desktop.*;
   import flash.display.*;
   import flash.errors.*;
   import flash.events.*;
   import flash.external.*;
   import flash.filters.*;
   import flash.geom.*;
   import flash.globalization.*;
   import flash.media.*;
   import flash.net.*;
   import flash.net.drm.*;
   import flash.printing.*;
   import flash.profiler.*;
   import flash.sampler.*;
   import flash.sensors.*;
   import flash.system.*;
   import flash.text.*;
   import flash.text.engine.*;
   import flash.text.ime.*;
   import flash.ui.*;
   import flash.utils.*;
   import flash.xml.*;
   import survarium.ammo.AmmoShareElement;
   
   public dynamic class SrvAmmoElement extends AmmoShareElement
   {
      
      public function SrvAmmoElement()
      {
         super();
         this.__setProp_stepper_storage_SrvAmmoElement_contewnt_0();
         this.__setProp_backpack_label_SrvAmmoElement_contewnt_0();
         this.__setProp_stepper_backpack_SrvAmmoElement_contewnt_0();
         this.__setProp_slider_SrvAmmoElement_contewnt_0();
      }
      
      internal function __setProp_stepper_storage_SrvAmmoElement_contewnt_0() : *
      {
         try
         {
            stepper_storage["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         stepper_storage.enabled = false;
         stepper_storage.enableInitCallback = false;
         stepper_storage.focusable = true;
         stepper_storage.maximum = 0;
         stepper_storage.minimum = 0;
         stepper_storage.stepSize = 1;
         stepper_storage.value = 0;
         stepper_storage.visible = true;
         try
         {
            stepper_storage["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_backpack_label_SrvAmmoElement_contewnt_0() : *
      {
         try
         {
            backpack_label["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         backpack_label.autoSize = "none";
         backpack_label.enabled = false;
         backpack_label.enableInitCallback = false;
         backpack_label.text = "";
         backpack_label.visible = true;
         try
         {
            backpack_label["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_stepper_backpack_SrvAmmoElement_contewnt_0() : *
      {
         try
         {
            stepper_backpack["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         stepper_backpack.enabled = false;
         stepper_backpack.enableInitCallback = false;
         stepper_backpack.focusable = true;
         stepper_backpack.maximum = 0;
         stepper_backpack.minimum = 0;
         stepper_backpack.stepSize = 1;
         stepper_backpack.value = 0;
         stepper_backpack.visible = false;
         try
         {
            stepper_backpack["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
      
      internal function __setProp_slider_SrvAmmoElement_contewnt_0() : *
      {
         try
         {
            slider["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         slider.enabled = true;
         slider.enableInitCallback = false;
         slider.focusable = true;
         slider.liveDragging = true;
         slider.maximum = 100;
         slider.minimum = 0;
         slider.offsetLeft = 1;
         slider.offsetRight = 1;
         slider.snapInterval = 0;
         slider.snapping = true;
         slider.value = 0;
         slider.visible = true;
         try
         {
            slider["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

